use axum::{
    Json, Router,
    extract::{ConnectInfo, DefaultBodyLimit, Multipart, Request, State},
    http::StatusCode,
    middleware as axum_mw,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use governor::{
    Quota, RateLimiter,
    clock::{Clock, DefaultClock},
};
use jsonwebtoken::Algorithm;
use serde_json::json;
use std::net::{IpAddr, SocketAddr};
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::{debug, info};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::config::{AuthMode, Config, LogFormat, LogOutput};
use crate::jwt::JwksCache;
use outlier::{
    CalculateRequest, CalculateResponse, ErrorResponse, PercentileMethod, calculate_percentile,
    read_values_from_bytes,
};

/// Type alias for the global (unkeyed) rate limiter
type GlobalLimiter =
    RateLimiter<governor::state::NotKeyed, governor::state::InMemoryState, DefaultClock>;

/// Type alias for the per-IP (keyed) rate limiter
type PerIpLimiter =
    RateLimiter<IpAddr, governor::state::keyed::DefaultKeyedStateStore<IpAddr>, DefaultClock>;

/// Shared application state
#[derive(Clone)]
struct AppState {
    auth_enabled: bool,
    auth_mode: AuthMode,
    api_keys: Vec<String>,
    jwks_cache: Option<Arc<JwksCache>>,
    global_limiter: Option<Arc<GlobalLimiter>>,
    per_ip_limiter: Option<Arc<PerIpLimiter>>,
}

#[derive(OpenApi)]
#[openapi(
    paths(
        calculate,
        calculate_file,
        health
    ),
    components(
        schemas(CalculateRequest, CalculateResponse, ErrorResponse, PercentileMethod)
    ),
    tags(
        (name = "outlier", description = "Percentile calculation API")
    ),
    info(
        title = "Outlier API",
        version = "0.1.0",
        description = "Calculate percentiles from numerical datasets via REST API",
        contact(
            name = "API Support",
            url = "https://github.com/wingnut128/outlier"
        ),
        license(
            name = "MIT",
            url = "https://opensource.org/licenses/MIT"
        )
    )
)]
struct ApiDoc;

/// Custom error type for API responses
struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let error_response = ErrorResponse {
            error: self.0.to_string(),
        };
        (StatusCode::BAD_REQUEST, Json(error_response)).into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

/// Calculate percentile from JSON array of values
#[utoipa::path(
    post,
    path = "/calculate",
    request_body = CalculateRequest,
    responses(
        (status = 200, description = "Percentile calculated successfully", body = CalculateResponse),
        (status = 400, description = "Invalid input", body = ErrorResponse)
    ),
    tag = "outlier"
)]
#[tracing::instrument(skip(payload), fields(percentile = %payload.percentile, value_count = %payload.values.len(), method = %payload.method))]
async fn calculate(
    Json(payload): Json<CalculateRequest>,
) -> Result<Json<CalculateResponse>, AppError> {
    let result = calculate_percentile(&payload.values, payload.percentile, payload.method)?;

    Ok(Json(CalculateResponse {
        count: payload.values.len(),
        percentile: payload.percentile,
        result,
        method: payload.method,
    }))
}

/// Calculate percentile from uploaded file (JSON or CSV)
///
/// Send a multipart form with:
/// - file: The data file (JSON array or CSV with "value" column)
/// - percentile: (optional) The percentile to calculate, defaults to 95
#[utoipa::path(
    post,
    path = "/calculate/file",
    request_body(content = String, description = "File upload (JSON or CSV)", content_type = "multipart/form-data"),
    responses(
        (status = 200, description = "Percentile calculated successfully", body = CalculateResponse),
        (status = 400, description = "Invalid input or file format", body = ErrorResponse)
    ),
    tag = "outlier"
)]
#[tracing::instrument(skip(multipart))]
async fn calculate_file(mut multipart: Multipart) -> Result<Json<CalculateResponse>, AppError> {
    let mut percentile = 95.0;
    let mut method = PercentileMethod::default();
    let mut file_data: Option<(String, Vec<u8>)> = None;

    // Process multipart fields
    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().map(|s| s.to_string()).unwrap_or_default();

        if name == "percentile" {
            if let Ok(text) = field.text().await
                && let Ok(p) = text.parse::<f64>()
            {
                percentile = p;
            }
        } else if name == "method" {
            if let Ok(text) = field.text().await
                && let Ok(m) = serde_json::from_value(serde_json::Value::String(text))
            {
                method = m;
            }
        } else if name == "file" {
            let filename = field
                .file_name()
                .map(|s| s.to_string())
                .unwrap_or_else(|| "data.json".to_string());
            if let Ok(bytes) = field.bytes().await {
                file_data = Some((filename, bytes.to_vec()));
            }
        }
    }

    // Validate we have file data
    let (filename, data) = file_data.ok_or_else(|| {
        AppError(anyhow::anyhow!(
            "No file provided. Send a file field with your data."
        ))
    })?;

    // Parse and calculate
    let values = read_values_from_bytes(&data, &filename)?;
    let result = calculate_percentile(&values, percentile, method)?;

    Ok(Json(CalculateResponse {
        count: values.len(),
        percentile,
        result,
        method,
    }))
}

/// Health check endpoint
#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Service is healthy", body = String)
    ),
    tag = "outlier"
)]
#[tracing::instrument]
async fn health() -> Json<serde_json::Value> {
    Json(json!({
        "status": "healthy",
        "service": "outlier",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// Constant-time comparison to prevent timing attacks on API key validation
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

/// Build a 401 Unauthorized response (generic — never reveals auth failure reason)
fn unauthorized_response() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({"error": "Unauthorized"})),
    )
        .into_response()
}

/// Check if request has a Bearer token in the Authorization header
fn has_bearer_token(request: &Request) -> bool {
    request
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|h| h.starts_with("Bearer "))
}

/// Check if request has an X-API-Key header
fn has_api_key_header(request: &Request) -> bool {
    request.headers().get("X-API-Key").is_some()
}

/// Validate request using static API key
async fn validate_api_key(state: &AppState, request: Request, next: axum_mw::Next) -> Response {
    let api_key = request
        .headers()
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok());

    match api_key {
        Some(key) => {
            let authorized = state
                .api_keys
                .iter()
                .any(|valid_key| constant_time_eq(key.as_bytes(), valid_key.as_bytes()));

            if authorized {
                next.run(request).await
            } else {
                unauthorized_response()
            }
        }
        None => unauthorized_response(),
    }
}

/// Validate request using JWT bearer token
async fn validate_jwt(state: &AppState, request: Request, next: axum_mw::Next) -> Response {
    let auth_header = request
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok());

    let token = match auth_header {
        Some(h) if h.starts_with("Bearer ") => &h[7..],
        _ => return unauthorized_response(),
    };

    match &state.jwks_cache {
        Some(cache) => match cache.validate_token(token).await {
            Ok(_claims) => next.run(request).await,
            Err(e) => {
                debug!("JWT validation failed: {e}");
                unauthorized_response()
            }
        },
        None => unauthorized_response(),
    }
}

/// Authentication middleware — dispatches to the configured auth mode
async fn auth_middleware(
    State(state): State<AppState>,
    request: Request,
    next: axum_mw::Next,
) -> Response {
    if !state.auth_enabled {
        return next.run(request).await;
    }

    match state.auth_mode {
        AuthMode::ApiKey => validate_api_key(&state, request, next).await,
        AuthMode::Jwt => validate_jwt(&state, request, next).await,
        AuthMode::Both => {
            // Try Bearer token first, then fall back to X-API-Key
            if has_bearer_token(&request) {
                validate_jwt(&state, request, next).await
            } else if has_api_key_header(&request) {
                validate_api_key(&state, request, next).await
            } else {
                unauthorized_response()
            }
        }
    }
}

/// Rate limiting middleware — checks global then per-IP limits
async fn rate_limit_middleware(
    State(state): State<AppState>,
    request: Request,
    next: axum_mw::Next,
) -> Response {
    // Check global rate limit
    if let Some(ref limiter) = state.global_limiter
        && let Err(not_until) = limiter.check()
    {
        let clock = DefaultClock::default();
        let wait = not_until.wait_time_from(clock.now());
        return too_many_requests_response(wait);
    }

    // Check per-IP rate limit (only when ConnectInfo is available)
    if let Some(ref limiter) = state.per_ip_limiter
        && let Some(connect_info) = request.extensions().get::<ConnectInfo<SocketAddr>>()
    {
        let ip = connect_info.0.ip();
        if let Err(not_until) = limiter.check_key(&ip) {
            let clock = DefaultClock::default();
            let wait = not_until.wait_time_from(clock.now());
            return too_many_requests_response(wait);
        }
    }

    next.run(request).await
}

/// Build a 429 Too Many Requests response with Retry-After header
fn too_many_requests_response(wait: std::time::Duration) -> Response {
    let retry_after = (wait.as_secs() + 1).to_string();
    let mut response = (
        StatusCode::TOO_MANY_REQUESTS,
        Json(json!({"error": "Too many requests"})),
    )
        .into_response();
    if let Ok(val) = axum::http::HeaderValue::from_str(&retry_after) {
        response.headers_mut().insert("retry-after", val);
    }
    response
}

/// Initialize logging based on configuration
fn init_logging(
    config: &Config,
) -> anyhow::Result<Option<tracing_appender::non_blocking::WorkerGuard>> {
    let level = config.logging.level.as_tracing_level();

    match &config.logging.output {
        LogOutput::File(path) => {
            let file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .map_err(|e| {
                    anyhow::anyhow!("Failed to open log file '{}': {}", path.display(), e)
                })?;
            let (non_blocking, guard) = tracing_appender::non_blocking(file);

            match config.logging.format {
                LogFormat::Json => {
                    tracing_subscriber::fmt()
                        .with_target(false)
                        .with_max_level(level)
                        .with_writer(non_blocking)
                        .json()
                        .init();
                }
                LogFormat::Pretty => {
                    tracing_subscriber::fmt()
                        .with_target(false)
                        .with_max_level(level)
                        .with_writer(non_blocking)
                        .pretty()
                        .init();
                }
                LogFormat::Compact => {
                    tracing_subscriber::fmt()
                        .with_target(false)
                        .with_max_level(level)
                        .with_writer(non_blocking)
                        .compact()
                        .init();
                }
            }
            Ok(Some(guard))
        }
        LogOutput::Stdout => {
            match config.logging.format {
                LogFormat::Json => {
                    tracing_subscriber::fmt()
                        .with_target(false)
                        .with_max_level(level)
                        .with_writer(std::io::stdout)
                        .json()
                        .init();
                }
                LogFormat::Pretty => {
                    tracing_subscriber::fmt()
                        .with_target(false)
                        .with_max_level(level)
                        .pretty()
                        .init();
                }
                LogFormat::Compact => {
                    tracing_subscriber::fmt()
                        .with_target(false)
                        .with_max_level(level)
                        .compact()
                        .init();
                }
            }
            Ok(None)
        }
        LogOutput::Stderr => {
            match config.logging.format {
                LogFormat::Json => {
                    tracing_subscriber::fmt()
                        .with_target(false)
                        .with_max_level(level)
                        .with_writer(std::io::stderr)
                        .json()
                        .init();
                }
                LogFormat::Pretty => {
                    tracing_subscriber::fmt()
                        .with_target(false)
                        .with_max_level(level)
                        .with_writer(std::io::stderr)
                        .pretty()
                        .init();
                }
                LogFormat::Compact => {
                    tracing_subscriber::fmt()
                        .with_target(false)
                        .with_max_level(level)
                        .with_writer(std::io::stderr)
                        .compact()
                        .init();
                }
            }
            Ok(None)
        }
    }
}

/// Build the application router with all endpoints and middleware
fn build_app(state: AppState) -> Router {
    // Public routes (no auth, no rate limit)
    let public_routes = Router::new()
        .route("/health", get(health))
        .merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()));

    // Protected routes (auth + rate limit middleware)
    let protected_routes = Router::new()
        .route("/calculate", post(calculate))
        .route("/calculate/file", post(calculate_file))
        .layer(axum_mw::from_fn_with_state(state.clone(), auth_middleware))
        .layer(axum_mw::from_fn_with_state(state, rate_limit_middleware));

    public_routes
        .merge(protected_routes)
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
}

/// Resolve API keys from environment variable or config file
fn resolve_api_keys(config: &Config) -> (Vec<String>, &'static str) {
    // Priority 1: OUTLIER_API_KEYS environment variable
    if let Ok(env_keys) = std::env::var("OUTLIER_API_KEYS") {
        let keys: Vec<String> = env_keys
            .split(',')
            .map(|k| k.trim().to_string())
            .filter(|k| !k.is_empty())
            .collect();
        if !keys.is_empty() {
            return (keys, "environment variable");
        }
    }

    // Priority 2: Config file
    if !config.auth.api_keys.is_empty() {
        return (config.auth.api_keys.clone(), "config file");
    }

    (Vec::new(), "none")
}

/// Parse algorithm strings into jsonwebtoken Algorithm values
fn parse_algorithms(alg_strings: &[String]) -> anyhow::Result<Vec<Algorithm>> {
    alg_strings
        .iter()
        .map(|s| match s.as_str() {
            "RS256" => Ok(Algorithm::RS256),
            "RS384" => Ok(Algorithm::RS384),
            "RS512" => Ok(Algorithm::RS512),
            "ES256" => Ok(Algorithm::ES256),
            "ES384" => Ok(Algorithm::ES384),
            other => anyhow::bail!("Unsupported JWT algorithm: {other}"),
        })
        .collect()
}

/// Start the API server
pub async fn serve(config: Config) -> anyhow::Result<()> {
    // Initialize tracing - keep guard alive for file logging
    let _guard = init_logging(&config)?;

    // Resolve API keys (needed for ApiKey and Both modes)
    let (api_keys, key_source) = resolve_api_keys(&config);

    if config.auth.enabled {
        match config.auth.mode {
            AuthMode::ApiKey => {
                if api_keys.is_empty() {
                    anyhow::bail!(
                        "Auth is enabled with api_key mode but no API keys found. \
                         Set OUTLIER_API_KEYS env var or auth.api_keys in config."
                    );
                }
                info!(
                    "API key authentication enabled ({} key(s) from {})",
                    api_keys.len(),
                    key_source
                );
            }
            AuthMode::Jwt => {
                info!("JWT authentication enabled");
            }
            AuthMode::Both => {
                if api_keys.is_empty() {
                    anyhow::bail!(
                        "Auth is enabled with both mode but no API keys found. \
                         Set OUTLIER_API_KEYS env var or auth.api_keys in config."
                    );
                }
                info!(
                    "Authentication enabled: API key ({} key(s) from {}) + JWT",
                    api_keys.len(),
                    key_source
                );
            }
        }
    } else {
        info!("Authentication disabled");
    }

    // Build JWKS cache if JWT mode is configured
    let jwks_cache =
        if config.auth.enabled && matches!(config.auth.mode, AuthMode::Jwt | AuthMode::Both) {
            // Resolve JWT settings from env vars (override config)
            let jwt_issuer = std::env::var("OUTLIER_JWT_ISSUER")
                .ok()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| config.auth.jwt.issuer.clone());

            let jwt_audience = std::env::var("OUTLIER_JWT_AUDIENCE")
                .ok()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| config.auth.jwt.audience.clone());

            let jwt_jwks_url = std::env::var("OUTLIER_JWT_JWKS_URL")
                .ok()
                .filter(|s| !s.is_empty())
                .or_else(|| config.auth.jwt.jwks_url.clone());

            if jwt_issuer.is_empty() {
                anyhow::bail!(
                    "JWT auth enabled but issuer is not configured. \
                     Set OUTLIER_JWT_ISSUER or auth.jwt.issuer in config."
                );
            }
            if jwt_audience.is_empty() {
                anyhow::bail!(
                    "JWT auth enabled but audience is not configured. \
                     Set OUTLIER_JWT_AUDIENCE or auth.jwt.audience in config."
                );
            }

            let jwks_url = jwt_jwks_url.unwrap_or_else(|| {
                let issuer = jwt_issuer.trim_end_matches('/');
                format!("{issuer}/.well-known/jwks.json")
            });

            let algorithms = parse_algorithms(&config.auth.jwt.algorithms)?;

            let cache = JwksCache::new(
                jwks_url,
                jwt_issuer.clone(),
                jwt_audience.clone(),
                algorithms,
                Duration::from_secs(config.auth.jwt.jwks_cache_ttl_secs),
            );

            // Eagerly fetch keys at startup to fail fast on misconfiguration
            cache.refresh_keys().await?;

            info!(
                "JWKS loaded (issuer: {}, audience: {})",
                jwt_issuer, jwt_audience
            );

            Some(Arc::new(cache))
        } else {
            None
        };

    // Build rate limiters
    let (global_limiter, per_ip_limiter) = if config.rate_limit.enabled {
        let global_quota = Quota::per_second(
            NonZeroU32::new(config.rate_limit.global_per_second)
                .ok_or_else(|| anyhow::anyhow!("global_per_second must be > 0"))?,
        )
        .allow_burst(
            NonZeroU32::new(config.rate_limit.global_burst)
                .ok_or_else(|| anyhow::anyhow!("global_burst must be > 0"))?,
        );

        let per_ip_quota = Quota::per_second(
            NonZeroU32::new(config.rate_limit.per_ip_per_second)
                .ok_or_else(|| anyhow::anyhow!("per_ip_per_second must be > 0"))?,
        )
        .allow_burst(
            NonZeroU32::new(config.rate_limit.per_ip_burst)
                .ok_or_else(|| anyhow::anyhow!("per_ip_burst must be > 0"))?,
        );

        info!(
            "Rate limiting enabled (per-IP: {}/s burst {}, global: {}/s burst {})",
            config.rate_limit.per_ip_per_second,
            config.rate_limit.per_ip_burst,
            config.rate_limit.global_per_second,
            config.rate_limit.global_burst,
        );

        (
            Some(Arc::new(RateLimiter::direct(global_quota))),
            Some(Arc::new(RateLimiter::keyed(per_ip_quota))),
        )
    } else {
        info!("Rate limiting disabled");
        (None, None)
    };

    let state = AppState {
        auth_enabled: config.auth.enabled,
        auth_mode: config.auth.mode,
        api_keys,
        jwks_cache,
        global_limiter,
        per_ip_limiter,
    };

    let app = build_app(state);

    let addr = SocketAddr::new(config.server.bind_ip, config.server.port);
    info!("Outlier API server listening on http://{}", addr);
    info!("API documentation available at http://{}/docs", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use jsonwebtoken::jwk::JwkSet;
    use jsonwebtoken::{EncodingKey, Header, encode};
    use tower::ServiceExt;

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

    fn test_app_state() -> AppState {
        AppState {
            auth_enabled: false,
            auth_mode: AuthMode::ApiKey,
            api_keys: Vec::new(),
            jwks_cache: None,
            global_limiter: None,
            per_ip_limiter: None,
        }
    }

    fn test_app_state_with_auth() -> AppState {
        AppState {
            auth_enabled: true,
            auth_mode: AuthMode::ApiKey,
            api_keys: vec!["test-api-key".to_string()],
            jwks_cache: None,
            global_limiter: None,
            per_ip_limiter: None,
        }
    }

    fn test_app_state_with_jwt() -> AppState {
        let jwks: JwkSet = serde_json::from_str(TEST_JWKS_JSON).unwrap();
        AppState {
            auth_enabled: true,
            auth_mode: AuthMode::Jwt,
            api_keys: Vec::new(),
            jwks_cache: Some(Arc::new(JwksCache::with_test_jwks(
                jwks,
                TEST_ISSUER.to_string(),
                TEST_AUDIENCE.to_string(),
            ))),
            global_limiter: None,
            per_ip_limiter: None,
        }
    }

    fn test_app_state_with_both() -> AppState {
        let jwks: JwkSet = serde_json::from_str(TEST_JWKS_JSON).unwrap();
        AppState {
            auth_enabled: true,
            auth_mode: AuthMode::Both,
            api_keys: vec!["test-api-key".to_string()],
            jwks_cache: Some(Arc::new(JwksCache::with_test_jwks(
                jwks,
                TEST_ISSUER.to_string(),
                TEST_AUDIENCE.to_string(),
            ))),
            global_limiter: None,
            per_ip_limiter: None,
        }
    }

    fn make_test_jwt(claims: &serde_json::Value) -> String {
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some("test-key-1".to_string());
        let key = EncodingKey::from_rsa_pem(TEST_RSA_PRIVATE_KEY.as_bytes()).unwrap();
        encode(&header, claims, &key).unwrap()
    }

    fn valid_jwt_claims() -> serde_json::Value {
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

    async fn response_json(response: Response) -> serde_json::Value {
        let body = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&body).unwrap()
    }

    // --- GET /health ---

    #[tokio::test]
    async fn health_returns_200() {
        let app = build_app(test_app_state());

        let response = app
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let json = response_json(response).await;
        assert_eq!(json["status"], "healthy");
        assert_eq!(json["service"], "outlier");
        assert!(json["version"].is_string());
    }

    // --- POST /calculate ---

    #[tokio::test]
    async fn calculate_returns_correct_percentile() {
        let app = build_app(test_app_state());

        let body = serde_json::json!({
            "values": [1.0, 2.0, 3.0, 4.0, 5.0],
            "percentile": 50.0
        });

        let response = app
            .oneshot(
                Request::post("/calculate")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let json = response_json(response).await;
        assert_eq!(json["count"], 5);
        assert_eq!(json["percentile"], 50.0);
        assert_eq!(json["result"], 3.0);
    }

    #[tokio::test]
    async fn calculate_defaults_to_95th_percentile() {
        let app = build_app(test_app_state());

        let body = serde_json::json!({
            "values": [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0]
        });

        let response = app
            .oneshot(
                Request::post("/calculate")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let json = response_json(response).await;
        assert_eq!(json["percentile"], 95.0);
    }

    #[tokio::test]
    async fn calculate_empty_values_returns_400() {
        let app = build_app(test_app_state());

        let body = serde_json::json!({
            "values": [],
            "percentile": 50.0
        });

        let response = app
            .oneshot(
                Request::post("/calculate")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let json = response_json(response).await;
        assert!(json["error"].as_str().unwrap().contains("empty dataset"));
    }

    #[tokio::test]
    async fn calculate_percentile_out_of_range_returns_400() {
        let app = build_app(test_app_state());

        let body = serde_json::json!({
            "values": [1.0, 2.0, 3.0],
            "percentile": 101.0
        });

        let response = app
            .oneshot(
                Request::post("/calculate")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let json = response_json(response).await;
        assert!(
            json["error"]
                .as_str()
                .unwrap()
                .contains("between 0 and 100")
        );
    }

    #[tokio::test]
    async fn calculate_invalid_json_returns_400() {
        let app = build_app(test_app_state());

        let response = app
            .oneshot(
                Request::post("/calculate")
                    .header("content-type", "application/json")
                    .body(Body::from("not valid json"))
                    .unwrap(),
            )
            .await
            .unwrap();

        // axum returns 400 for JSON syntax errors
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn calculate_missing_content_type_returns_415() {
        let app = build_app(test_app_state());

        let body = serde_json::json!({
            "values": [1.0, 2.0, 3.0],
            "percentile": 50.0
        });

        let response = app
            .oneshot(
                Request::post("/calculate")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }

    // --- POST /calculate/file (JSON upload) ---

    fn multipart_body(boundary: &str, filename: &str, content: &[u8]) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(
            format!(
                "Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n\
                 Content-Type: application/octet-stream\r\n\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(content);
        body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
        body
    }

    fn multipart_body_with_percentile(
        boundary: &str,
        filename: &str,
        content: &[u8],
        percentile: f64,
    ) -> Vec<u8> {
        let mut body = Vec::new();
        // percentile field
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"percentile\"\r\n\r\n{percentile}\r\n")
                .as_bytes(),
        );
        // file field
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(
            format!(
                "Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n\
                 Content-Type: application/octet-stream\r\n\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(content);
        body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
        body
    }

    #[tokio::test]
    async fn calculate_file_json_upload() {
        let app = build_app(test_app_state());
        let boundary = "test-boundary";
        let json_data = b"[1.0, 2.0, 3.0, 4.0, 5.0]";
        let body = multipart_body(boundary, "data.json", json_data);

        let response = app
            .oneshot(
                Request::post("/calculate/file")
                    .header(
                        "content-type",
                        format!("multipart/form-data; boundary={boundary}"),
                    )
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let json = response_json(response).await;
        assert_eq!(json["count"], 5);
        assert_eq!(json["percentile"], 95.0); // default
    }

    #[tokio::test]
    async fn calculate_file_csv_upload() {
        let app = build_app(test_app_state());
        let boundary = "test-boundary";
        let csv_data = b"value\n1.0\n2.0\n3.0\n4.0\n5.0\n";
        let body = multipart_body(boundary, "data.csv", csv_data);

        let response = app
            .oneshot(
                Request::post("/calculate/file")
                    .header(
                        "content-type",
                        format!("multipart/form-data; boundary={boundary}"),
                    )
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let json = response_json(response).await;
        assert_eq!(json["count"], 5);
        assert_eq!(json["percentile"], 95.0);
    }

    #[tokio::test]
    async fn calculate_file_with_custom_percentile() {
        let app = build_app(test_app_state());
        let boundary = "test-boundary";
        let json_data = b"[1.0, 2.0, 3.0, 4.0, 5.0]";
        let body = multipart_body_with_percentile(boundary, "data.json", json_data, 50.0);

        let response = app
            .oneshot(
                Request::post("/calculate/file")
                    .header(
                        "content-type",
                        format!("multipart/form-data; boundary={boundary}"),
                    )
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let json = response_json(response).await;
        assert_eq!(json["percentile"], 50.0);
        assert_eq!(json["result"], 3.0);
    }

    #[tokio::test]
    async fn calculate_file_unsupported_format_returns_400() {
        let app = build_app(test_app_state());
        let boundary = "test-boundary";
        let body = multipart_body(boundary, "data.xml", b"<values><v>1</v></values>");

        let response = app
            .oneshot(
                Request::post("/calculate/file")
                    .header(
                        "content-type",
                        format!("multipart/form-data; boundary={boundary}"),
                    )
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let json = response_json(response).await;
        assert!(
            json["error"]
                .as_str()
                .unwrap()
                .contains("Unsupported file format")
        );
    }

    #[tokio::test]
    async fn calculate_file_no_file_returns_400() {
        let app = build_app(test_app_state());
        let boundary = "test-boundary";
        // Send a multipart body with only a percentile field, no file
        let body = format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"percentile\"\r\n\r\n\
             50.0\r\n\
             --{boundary}--\r\n"
        );

        let response = app
            .oneshot(
                Request::post("/calculate/file")
                    .header(
                        "content-type",
                        format!("multipart/form-data; boundary={boundary}"),
                    )
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let json = response_json(response).await;
        assert!(json["error"].as_str().unwrap().contains("No file provided"));
    }

    #[tokio::test]
    async fn calculate_file_invalid_json_returns_400() {
        let app = build_app(test_app_state());
        let boundary = "test-boundary";
        let body = multipart_body(boundary, "bad.json", b"not valid json");

        let response = app
            .oneshot(
                Request::post("/calculate/file")
                    .header(
                        "content-type",
                        format!("multipart/form-data; boundary={boundary}"),
                    )
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let json = response_json(response).await;
        assert!(
            json["error"]
                .as_str()
                .unwrap()
                .contains("Failed to parse JSON")
        );
    }

    #[tokio::test]
    async fn calculate_file_invalid_csv_returns_400() {
        let app = build_app(test_app_state());
        let boundary = "test-boundary";
        // CSV with wrong header
        let body = multipart_body(boundary, "bad.csv", b"wrong_header\n1.0\n2.0\n");

        let response = app
            .oneshot(
                Request::post("/calculate/file")
                    .header(
                        "content-type",
                        format!("multipart/form-data; boundary={boundary}"),
                    )
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    // --- Method selection tests ---

    #[tokio::test]
    async fn calculate_returns_method_in_response() {
        let app = build_app(test_app_state());

        let body = serde_json::json!({
            "values": [1.0, 2.0, 3.0, 4.0, 5.0],
            "percentile": 50.0
        });

        let response = app
            .oneshot(
                Request::post("/calculate")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let json = response_json(response).await;
        assert_eq!(json["method"], "linear");
    }

    #[tokio::test]
    async fn calculate_with_explicit_method() {
        let app = build_app(test_app_state());

        let body = serde_json::json!({
            "values": [1.0, 2.0, 3.0, 4.0, 5.0],
            "percentile": 40.0,
            "method": "nearest_rank"
        });

        let response = app
            .oneshot(
                Request::post("/calculate")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let json = response_json(response).await;
        assert_eq!(json["method"], "nearest_rank");
        // nearest_rank at P40: index=1.6, round→2, sorted[2]=3.0
        assert_eq!(json["result"], 3.0);
    }

    #[tokio::test]
    async fn calculate_with_invalid_method_returns_client_error() {
        let app = build_app(test_app_state());

        let body = serde_json::json!({
            "values": [1.0, 2.0, 3.0],
            "percentile": 50.0,
            "method": "bogus"
        });

        let response = app
            .oneshot(
                Request::post("/calculate")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert!(response.status().is_client_error());
    }

    #[tokio::test]
    async fn calculate_file_with_method_field() {
        let app = build_app(test_app_state());
        let boundary = "test-boundary";
        let json_data = b"[1.0, 2.0, 3.0, 4.0, 5.0]";

        // Build multipart body with method field
        let mut body = Vec::new();
        // method field
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"method\"\r\n\r\nlower\r\n");
        // percentile field
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(
            b"Content-Disposition: form-data; name=\"percentile\"\r\n\r\n40\r\n",
        );
        // file field
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(
            b"Content-Disposition: form-data; name=\"file\"; filename=\"data.json\"\r\n\
              Content-Type: application/octet-stream\r\n\r\n",
        );
        body.extend_from_slice(json_data);
        body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

        let response = app
            .oneshot(
                Request::post("/calculate/file")
                    .header(
                        "content-type",
                        format!("multipart/form-data; boundary={boundary}"),
                    )
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let json = response_json(response).await;
        assert_eq!(json["method"], "lower");
        // lower at P40: floor(1.6)=1, sorted[1]=2.0
        assert_eq!(json["result"], 2.0);
    }

    // --- API Key Authentication tests ---

    #[tokio::test]
    async fn auth_returns_401_without_key() {
        let app = build_app(test_app_state_with_auth());

        let body = serde_json::json!({
            "values": [1.0, 2.0, 3.0],
            "percentile": 50.0
        });

        let response = app
            .oneshot(
                Request::post("/calculate")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let json = response_json(response).await;
        assert_eq!(json["error"], "Unauthorized");
    }

    #[tokio::test]
    async fn auth_returns_401_with_invalid_key() {
        let app = build_app(test_app_state_with_auth());

        let body = serde_json::json!({
            "values": [1.0, 2.0, 3.0],
            "percentile": 50.0
        });

        let response = app
            .oneshot(
                Request::post("/calculate")
                    .header("content-type", "application/json")
                    .header("X-API-Key", "wrong-key")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let json = response_json(response).await;
        assert_eq!(json["error"], "Unauthorized");
    }

    #[tokio::test]
    async fn auth_returns_200_with_valid_key() {
        let app = build_app(test_app_state_with_auth());

        let body = serde_json::json!({
            "values": [1.0, 2.0, 3.0, 4.0, 5.0],
            "percentile": 50.0
        });

        let response = app
            .oneshot(
                Request::post("/calculate")
                    .header("content-type", "application/json")
                    .header("X-API-Key", "test-api-key")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let json = response_json(response).await;
        assert_eq!(json["result"], 3.0);
    }

    #[tokio::test]
    async fn auth_disabled_allows_requests_without_key() {
        let app = build_app(test_app_state());

        let body = serde_json::json!({
            "values": [1.0, 2.0, 3.0],
            "percentile": 50.0
        });

        let response = app
            .oneshot(
                Request::post("/calculate")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn health_accessible_without_auth() {
        let app = build_app(test_app_state_with_auth());

        let response = app
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let json = response_json(response).await;
        assert_eq!(json["status"], "healthy");
    }

    #[tokio::test]
    async fn auth_error_does_not_reveal_key_info() {
        let app = build_app(test_app_state_with_auth());

        // Missing key
        let response = app
            .oneshot(
                Request::post("/calculate")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"values":[1],"percentile":50}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        let json = response_json(response).await;
        let error_msg = json["error"].as_str().unwrap();
        assert_eq!(error_msg, "Unauthorized");
        assert!(!error_msg.contains("key"));
        assert!(!error_msg.contains("missing"));
        assert!(!error_msg.contains("invalid"));
    }

    #[tokio::test]
    async fn auth_file_endpoint_requires_key() {
        let app = build_app(test_app_state_with_auth());
        let boundary = "test-boundary";
        let json_data = b"[1.0, 2.0, 3.0]";
        let body = multipart_body(boundary, "data.json", json_data);

        let response = app
            .oneshot(
                Request::post("/calculate/file")
                    .header(
                        "content-type",
                        format!("multipart/form-data; boundary={boundary}"),
                    )
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn auth_file_endpoint_works_with_valid_key() {
        let app = build_app(test_app_state_with_auth());
        let boundary = "test-boundary";
        let json_data = b"[1.0, 2.0, 3.0]";
        let body = multipart_body(boundary, "data.json", json_data);

        let response = app
            .oneshot(
                Request::post("/calculate/file")
                    .header(
                        "content-type",
                        format!("multipart/form-data; boundary={boundary}"),
                    )
                    .header("X-API-Key", "test-api-key")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    // --- JWT Authentication tests ---

    #[tokio::test]
    async fn jwt_returns_401_without_bearer() {
        let app = build_app(test_app_state_with_jwt());

        let response = app
            .oneshot(
                Request::post("/calculate")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"values":[1,2,3],"percentile":50}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn jwt_returns_401_with_invalid_bearer() {
        let app = build_app(test_app_state_with_jwt());

        let response = app
            .oneshot(
                Request::post("/calculate")
                    .header("content-type", "application/json")
                    .header("Authorization", "Bearer garbage.not.jwt")
                    .body(Body::from(r#"{"values":[1,2,3],"percentile":50}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn jwt_returns_200_with_valid_bearer() {
        let app = build_app(test_app_state_with_jwt());
        let token = make_test_jwt(&valid_jwt_claims());

        let response = app
            .oneshot(
                Request::post("/calculate")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::from(r#"{"values":[1,2,3],"percentile":50}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn jwt_returns_401_with_expired_token() {
        let app = build_app(test_app_state_with_jwt());
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let claims = serde_json::json!({
            "sub": "user123",
            "iss": TEST_ISSUER,
            "aud": TEST_AUDIENCE,
            "exp": now - 3600,
            "iat": now - 7200,
        });
        let token = make_test_jwt(&claims);

        let response = app
            .oneshot(
                Request::post("/calculate")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::from(r#"{"values":[1,2,3],"percentile":50}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn jwt_error_does_not_reveal_details() {
        let app = build_app(test_app_state_with_jwt());

        let response = app
            .oneshot(
                Request::post("/calculate")
                    .header("content-type", "application/json")
                    .header("Authorization", "Bearer bad.token.here")
                    .body(Body::from(r#"{"values":[1,2,3],"percentile":50}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        let json = response_json(response).await;
        assert_eq!(json["error"], "Unauthorized");
    }

    #[tokio::test]
    async fn health_accessible_without_jwt() {
        let app = build_app(test_app_state_with_jwt());

        let response = app
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    // --- Both mode tests ---

    #[tokio::test]
    async fn both_mode_accepts_api_key() {
        let app = build_app(test_app_state_with_both());

        let response = app
            .oneshot(
                Request::post("/calculate")
                    .header("content-type", "application/json")
                    .header("X-API-Key", "test-api-key")
                    .body(Body::from(r#"{"values":[1,2,3],"percentile":50}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn both_mode_accepts_bearer_jwt() {
        let app = build_app(test_app_state_with_both());
        let token = make_test_jwt(&valid_jwt_claims());

        let response = app
            .oneshot(
                Request::post("/calculate")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::from(r#"{"values":[1,2,3],"percentile":50}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn both_mode_rejects_no_credentials() {
        let app = build_app(test_app_state_with_both());

        let response = app
            .oneshot(
                Request::post("/calculate")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"values":[1,2,3],"percentile":50}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn api_key_mode_ignores_bearer_header() {
        let app = build_app(test_app_state_with_auth());
        let token = make_test_jwt(&valid_jwt_claims());

        // In ApiKey mode, Bearer token is not recognized — only X-API-Key works
        let response = app
            .oneshot(
                Request::post("/calculate")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::from(r#"{"values":[1,2,3],"percentile":50}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    // --- Rate limiting tests ---

    #[tokio::test]
    async fn global_rate_limit_returns_429() {
        let state = AppState {
            auth_enabled: false,
            auth_mode: AuthMode::ApiKey,
            api_keys: Vec::new(),
            jwks_cache: None,
            global_limiter: Some(Arc::new(RateLimiter::direct(Quota::per_second(
                NonZeroU32::new(1).unwrap(),
            )))),
            per_ip_limiter: None,
        };
        let app = build_app(state);

        // First request should succeed
        let response = app
            .clone()
            .oneshot(
                Request::post("/calculate")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"values":[1,2,3],"percentile":50}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Second request should be rate limited
        let response = app
            .oneshot(
                Request::post("/calculate")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"values":[1,2,3],"percentile":50}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn rate_limit_includes_retry_after_header() {
        let state = AppState {
            auth_enabled: false,
            auth_mode: AuthMode::ApiKey,
            api_keys: Vec::new(),
            jwks_cache: None,
            global_limiter: Some(Arc::new(RateLimiter::direct(Quota::per_second(
                NonZeroU32::new(1).unwrap(),
            )))),
            per_ip_limiter: None,
        };
        let app = build_app(state);

        // Exhaust the limit
        let _ = app
            .clone()
            .oneshot(
                Request::post("/calculate")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"values":[1,2,3],"percentile":50}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Check the 429 response has Retry-After
        let response = app
            .oneshot(
                Request::post("/calculate")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"values":[1,2,3],"percentile":50}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert!(response.headers().contains_key("retry-after"));
    }

    #[tokio::test]
    async fn health_not_rate_limited() {
        let state = AppState {
            auth_enabled: false,
            auth_mode: AuthMode::ApiKey,
            api_keys: Vec::new(),
            jwks_cache: None,
            global_limiter: Some(Arc::new(RateLimiter::direct(Quota::per_second(
                NonZeroU32::new(1).unwrap(),
            )))),
            per_ip_limiter: None,
        };
        let app = build_app(state);

        // Health endpoint should always succeed, even after rate limit is exhausted
        for _ in 0..5 {
            let response = app
                .clone()
                .oneshot(Request::get("/health").body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
        }
    }

    #[tokio::test]
    async fn rate_limit_disabled_allows_all_requests() {
        let app = build_app(test_app_state());

        // Multiple requests should all succeed with no rate limiter
        for _ in 0..5 {
            let response = app
                .clone()
                .oneshot(
                    Request::post("/calculate")
                        .header("content-type", "application/json")
                        .body(Body::from(r#"{"values":[1,2,3],"percentile":50}"#))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
        }
    }

    #[tokio::test]
    async fn rate_limit_runs_before_auth() {
        // Rate limit should reject before auth checks, throttling brute-force attempts
        let state = AppState {
            auth_enabled: true,
            auth_mode: AuthMode::ApiKey,
            api_keys: vec!["valid-key".to_string()],
            jwks_cache: None,
            global_limiter: Some(Arc::new(RateLimiter::direct(Quota::per_second(
                NonZeroU32::new(1).unwrap(),
            )))),
            per_ip_limiter: None,
        };
        let app = build_app(state);

        // First request with wrong key → 401 (rate limit passes, auth fails)
        let response = app
            .clone()
            .oneshot(
                Request::post("/calculate")
                    .header("content-type", "application/json")
                    .header("X-API-Key", "wrong-key")
                    .body(Body::from(r#"{"values":[1,2,3],"percentile":50}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // Second request → 429 (rate limit triggers before auth)
        let response = app
            .oneshot(
                Request::post("/calculate")
                    .header("content-type", "application/json")
                    .header("X-API-Key", "wrong-key")
                    .body(Body::from(r#"{"values":[1,2,3],"percentile":50}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    // --- constant_time_eq tests ---

    #[test]
    fn constant_time_eq_equal_strings() {
        assert!(constant_time_eq(b"hello", b"hello"));
    }

    #[test]
    fn constant_time_eq_different_strings() {
        assert!(!constant_time_eq(b"hello", b"world"));
    }

    #[test]
    fn constant_time_eq_different_lengths() {
        assert!(!constant_time_eq(b"short", b"longer"));
    }

    #[test]
    fn constant_time_eq_empty_strings() {
        assert!(constant_time_eq(b"", b""));
    }

    // --- resolve_api_keys tests ---

    #[test]
    fn resolve_api_keys_from_config() {
        let mut config = Config::default();
        config.auth.api_keys = vec!["key1".to_string(), "key2".to_string()];

        // SAFETY: test-only; no concurrent env var access in this test
        unsafe { std::env::remove_var("OUTLIER_API_KEYS") };

        let (keys, source) = resolve_api_keys(&config);
        assert_eq!(keys, vec!["key1", "key2"]);
        assert_eq!(source, "config file");
    }

    #[test]
    fn resolve_api_keys_empty_when_none_configured() {
        let config = Config::default();
        // SAFETY: test-only; no concurrent env var access in this test
        unsafe { std::env::remove_var("OUTLIER_API_KEYS") };

        let (keys, source) = resolve_api_keys(&config);
        assert!(keys.is_empty());
        assert_eq!(source, "none");
    }

    // --- parse_algorithms tests ---

    #[test]
    fn parse_valid_algorithms() {
        let algs = vec!["RS256".to_string(), "RS384".to_string()];
        let result = parse_algorithms(&algs).unwrap();
        assert_eq!(result, vec![Algorithm::RS256, Algorithm::RS384]);
    }

    #[test]
    fn parse_unsupported_algorithm_fails() {
        let algs = vec!["HS256".to_string()];
        assert!(parse_algorithms(&algs).is_err());
    }
}
