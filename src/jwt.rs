use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header, jwk::JwkSet};
use reqwest::Client;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::debug;

const MIN_REFRESH_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub enum JwtError {
    MissingKid,
    KeyNotFound(String),
    FetchError(String),
    ValidationError(String),
    InvalidHeader(String),
}

impl std::fmt::Display for JwtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JwtError::MissingKid => write!(f, "JWT missing kid header"),
            JwtError::KeyNotFound(kid) => write!(f, "Key not found: {kid}"),
            JwtError::FetchError(msg) => write!(f, "JWKS fetch error: {msg}"),
            JwtError::ValidationError(msg) => write!(f, "JWT validation error: {msg}"),
            JwtError::InvalidHeader(msg) => write!(f, "Invalid JWT header: {msg}"),
        }
    }
}

impl std::error::Error for JwtError {}

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
pub struct Claims {
    pub sub: Option<String>,
    pub iss: Option<String>,
    pub exp: Option<u64>,
    pub iat: Option<u64>,
}

struct CachedKeys {
    keys: JwkSet,
    fetched_at: Instant,
    last_refresh_attempt: Instant,
}

pub struct JwksCache {
    jwks_url: String,
    issuer: String,
    audience: String,
    algorithms: Vec<Algorithm>,
    cache: RwLock<CachedKeys>,
    ttl: Duration,
    http_client: Client,
}

impl JwksCache {
    pub fn new(
        jwks_url: String,
        issuer: String,
        audience: String,
        algorithms: Vec<Algorithm>,
        ttl: Duration,
    ) -> Self {
        Self {
            jwks_url,
            issuer,
            audience,
            algorithms,
            cache: RwLock::new(CachedKeys {
                keys: JwkSet { keys: vec![] },
                fetched_at: Instant::now() - ttl,
                last_refresh_attempt: Instant::now() - MIN_REFRESH_INTERVAL,
            }),
            ttl,
            http_client: Client::new(),
        }
    }

    pub async fn validate_token(&self, token: &str) -> Result<Claims, JwtError> {
        let header = decode_header(token).map_err(|e| JwtError::InvalidHeader(e.to_string()))?;

        let kid = header.kid.as_deref().ok_or(JwtError::MissingKid)?;

        // Refresh if cache is stale
        {
            let cache = self.cache.read().await;
            if cache.fetched_at.elapsed() > self.ttl {
                drop(cache);
                self.try_refresh().await?;
            }
        }

        // Try to validate with current cache
        match self.validate_with_kid(token, kid).await {
            Ok(claims) => Ok(claims),
            Err(JwtError::KeyNotFound(_)) => {
                // Key not found — maybe IdP rotated keys, try refreshing
                self.try_refresh().await?;
                self.validate_with_kid(token, kid).await
            }
            Err(e) => Err(e),
        }
    }

    async fn validate_with_kid(&self, token: &str, kid: &str) -> Result<Claims, JwtError> {
        let cache = self.cache.read().await;

        let jwk = cache
            .keys
            .keys
            .iter()
            .find(|k| k.common.key_id.as_deref() == Some(kid))
            .ok_or_else(|| JwtError::KeyNotFound(kid.to_string()))?;

        let decoding_key =
            DecodingKey::from_jwk(jwk).map_err(|e| JwtError::ValidationError(e.to_string()))?;

        let mut validation =
            Validation::new(self.algorithms.first().copied().unwrap_or(Algorithm::RS256));
        validation.set_issuer(&[&self.issuer]);
        validation.set_audience(&[&self.audience]);
        validation.algorithms = self.algorithms.clone();

        let token_data = decode::<Claims>(token, &decoding_key, &validation)
            .map_err(|e| JwtError::ValidationError(e.to_string()))?;

        Ok(token_data.claims)
    }

    async fn try_refresh(&self) -> Result<(), JwtError> {
        {
            let cache = self.cache.read().await;
            if cache.last_refresh_attempt.elapsed() < MIN_REFRESH_INTERVAL {
                return Ok(());
            }
        }
        self.refresh_keys().await
    }

    pub async fn refresh_keys(&self) -> Result<(), JwtError> {
        debug!(jwks_url = %self.jwks_url, "Refreshing JWKS keys");

        let response = self
            .http_client
            .get(&self.jwks_url)
            .send()
            .await
            .map_err(|e| JwtError::FetchError(e.to_string()))?;

        let jwks: JwkSet = response
            .json()
            .await
            .map_err(|e| JwtError::FetchError(e.to_string()))?;

        debug!(key_count = jwks.keys.len(), "JWKS keys refreshed");

        let mut cache = self.cache.write().await;
        cache.keys = jwks;
        cache.fetched_at = Instant::now();
        cache.last_refresh_attempt = Instant::now();

        Ok(())
    }
}

#[cfg(test)]
impl JwksCache {
    pub fn with_test_jwks(jwks: JwkSet, issuer: String, audience: String) -> Self {
        let now = Instant::now();
        Self {
            jwks_url: String::new(),
            issuer,
            audience,
            algorithms: vec![Algorithm::RS256],
            cache: RwLock::new(CachedKeys {
                keys: jwks,
                fetched_at: now,
                last_refresh_attempt: now,
            }),
            ttl: Duration::from_secs(3600),
            http_client: Client::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{EncodingKey, Header, encode};

    const TEST_ISSUER: &str = "https://test.example.com/";
    const TEST_AUDIENCE: &str = "https://api.outlier.dev";

    const TEST_RSA_PRIVATE_KEY: &str = "-----BEGIN PRIVATE KEY-----
MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQDxJpq2+xb6FYw3
huidUHc+7p8J+hWD1tMLwEG9WCjSHBxrmyNfgZ5WvJbUzLQhRnAw1jh+Z8G4x0lt
zDCitpMPOVbdQjlUZsXhInKQcfA7oo9b+veegk8bVb9t5v0Z4PA9hVUxKjpW5NsO
5rYM1NfZccANEQg+hyJ667YXZcO9cPV/R/ZIoAtlY1Y7VxGNNo81/I3Xc0Rpoy64
mzrC2D5US8cOKaym9NZ9+QKa58BRGQCvseadMwPbM7qLpzdrIAN+l2ank4PgbfFF
YEVzpDTy535k2XRfaoIZxQMPCdXYA0AEOmU2VQFQpmU8GsxyGLfulQ1VwvWfgMdg
EoxcgLINAgMBAAECggEAMXDtQnu7S720OEQwF+TB9gSdVcHQvG2EaoHZ2JSlFeLO
jt9JQtED4hubPvjTK4lSAilBfuUN6jDtpJW7GPkesH3cidOEhoHlqxFRdLzveIKN
KtoK/5QO3PdZHpK/rJkaGDroawKR4HPeV7FEfN/8eyffrK4jxxIUpygBds2V8pY6
y2tdgQXZlKNoK/L/jsSSTWLxKrqSxcMrET+6EkLwUp5VU7Sqc1ITrzXtzVgCf//M
Abgdul6Y2EhNBhzz8RpSRYdnm3j9Cr6PFOL/ykj1Xz19Idq1954lQYjvSrPOpd4v
wwr+t08foaPVOUUU5cEofBmY3nD0e79gGJEGuYWmvwKBgQD/TglcRaUD2CNUDKlo
pOXNfdXAf3aDK1XbD0zar+h7vrwYYBqCs5BEHTprHtndutyy5JTKxMuNO5iloWyH
kDupvnKsPuPQB55xOEeeNL+D/A5INiNUxB/+gKHi/7WPGuf52O7k2gWimVrIFScV
h987ViDp6kkO6JqKIopm0v3W7wKBgQDxzrOYbrJ3PPtT3Q3KGf4a8/GFQ7hnWM4p
4XBgx3Wmhzih1AWKEj98YFBLzXNQgrN1Z0adpp0SwIuAGM3AtLvztTHEXcIDA/Z0
3krG+1tRtQPTPtLILXXXjznqWRlSKrqNG2ZyQ/HhTpBjAuExNQE/mnFu+Nv07u+K
NFFMWKmmwwKBgCRqeSNUO8lklwVyGOf4PV8mR8sBY2IqWEC62feHh92+ww2nB6EF
A9rzYFXPPLxH3xsVR7P0hiRLD+bwM47Sn/ACXlD7V3tg2tTDdlO2qmqlFVRvhHKe
1wFyT6UVXExhRh15N/okrxEWVsCbY8vKaakJDADRjkI2I3T4oE0yY0q5AoGATtnj
sNJwOffV0RwlkgD13t4rpRRXPsQzvm54UebZE6vGqObVw5d9wlY5+O4PK3LjiGZc
Ha6mS+Yj12q/NZb6L1en2evlB0y0gpm2crqmpbdMfwdefs5sPhXDggr5+dRbLwZ/
WsWTS7Bt3wuiWYR6Wr5HPTPDrlR4Im47EJVdBTcCgYEA68JlknR6PYfKm5X4FS/P
dCS+5eowpEh59cuAu3IgRiZdaMBmSE+37oMbepUNAdrvsIFbrTi5YeMWaJAjs8MV
YnM8TwJ7Xmw+fOb37qfQpOrf1kUucToMfpNM2e4nNrcQT2S4KcetKEPor6W2rcGU
M9LEGJLcpr1rIhS7lm02vRk=
-----END PRIVATE KEY-----";

    const TEST_JWKS_JSON: &str = r#"{"keys":[{"kty":"RSA","n":"8SaatvsW-hWMN4bonVB3Pu6fCfoVg9bTC8BBvVgo0hwca5sjX4GeVryW1My0IUZwMNY4fmfBuMdJbcwworaTDzlW3UI5VGbF4SJykHHwO6KPW_r3noJPG1W_beb9GeDwPYVVMSo6VuTbDua2DNTX2XHADREIPocieuu2F2XDvXD1f0f2SKALZWNWO1cRjTaPNfyN13NEaaMuuJs6wtg-VEvHDimspvTWffkCmufAURkAr7HmnTMD2zO6i6c3ayADfpdmp5OD4G3xRWBFc6Q08ud-ZNl0X2qCGcUDDwnV2ANABDplNlUBUKZlPBrMchi37pUNVcL1n4DHYBKMXICyDQ","e":"AQAB","kid":"test-key-1","use":"sig","alg":"RS256"}]}"#;

    fn make_test_jwt(claims: &serde_json::Value, kid: &str) -> String {
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(kid.to_string());
        let key = EncodingKey::from_rsa_pem(TEST_RSA_PRIVATE_KEY.as_bytes()).unwrap();
        encode(&header, claims, &key).unwrap()
    }

    fn valid_claims() -> serde_json::Value {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        serde_json::json!({
            "sub": "user123",
            "iss": TEST_ISSUER,
            "aud": TEST_AUDIENCE,
            "exp": now + 3600,
            "iat": now,
        })
    }

    fn test_cache() -> JwksCache {
        let jwks: JwkSet = serde_json::from_str(TEST_JWKS_JSON).unwrap();
        JwksCache::with_test_jwks(jwks, TEST_ISSUER.to_string(), TEST_AUDIENCE.to_string())
    }

    #[tokio::test]
    async fn validate_valid_token() {
        let cache = test_cache();
        let token = make_test_jwt(&valid_claims(), "test-key-1");
        let claims = cache.validate_token(&token).await.unwrap();
        assert_eq!(claims.sub.as_deref(), Some("user123"));
    }

    #[tokio::test]
    async fn reject_expired_token() {
        let cache = test_cache();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let claims = serde_json::json!({
            "sub": "user123",
            "iss": TEST_ISSUER,
            "aud": TEST_AUDIENCE,
            "exp": now - 3600, // expired 1 hour ago
            "iat": now - 7200,
        });
        let token = make_test_jwt(&claims, "test-key-1");
        assert!(cache.validate_token(&token).await.is_err());
    }

    #[tokio::test]
    async fn reject_wrong_issuer() {
        let cache = test_cache();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let claims = serde_json::json!({
            "sub": "user123",
            "iss": "https://wrong-issuer.example.com/",
            "aud": TEST_AUDIENCE,
            "exp": now + 3600,
            "iat": now,
        });
        let token = make_test_jwt(&claims, "test-key-1");
        assert!(cache.validate_token(&token).await.is_err());
    }

    #[tokio::test]
    async fn reject_wrong_audience() {
        let cache = test_cache();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let claims = serde_json::json!({
            "sub": "user123",
            "iss": TEST_ISSUER,
            "aud": "https://wrong-audience.example.com",
            "exp": now + 3600,
            "iat": now,
        });
        let token = make_test_jwt(&claims, "test-key-1");
        assert!(cache.validate_token(&token).await.is_err());
    }

    #[tokio::test]
    async fn reject_unknown_kid() {
        let cache = test_cache();
        let token = make_test_jwt(&valid_claims(), "unknown-key-id");
        assert!(cache.validate_token(&token).await.is_err());
    }

    #[tokio::test]
    async fn reject_missing_kid() {
        let cache = test_cache();
        // Create a token without kid
        let header = Header::new(Algorithm::RS256);
        let key = EncodingKey::from_rsa_pem(TEST_RSA_PRIVATE_KEY.as_bytes()).unwrap();
        let token = encode(&header, &valid_claims(), &key).unwrap();
        let err = cache.validate_token(&token).await.unwrap_err();
        assert!(matches!(err, JwtError::MissingKid));
    }

    #[tokio::test]
    async fn reject_garbage_token() {
        let cache = test_cache();
        assert!(cache.validate_token("not.a.jwt").await.is_err());
    }
}
