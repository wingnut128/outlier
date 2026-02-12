use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Multipart},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde_json::json;
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::info;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::config::{Config, LogFormat, LogOutput};
use outlier::{
    CalculateRequest, CalculateResponse, ErrorResponse, calculate_percentile,
    read_values_from_bytes,
};

#[derive(OpenApi)]
#[openapi(
    paths(
        calculate,
        calculate_file,
        health
    ),
    components(
        schemas(CalculateRequest, CalculateResponse, ErrorResponse)
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
#[tracing::instrument(skip(payload), fields(percentile = %payload.percentile, value_count = %payload.values.len()))]
async fn calculate(
    Json(payload): Json<CalculateRequest>,
) -> Result<Json<CalculateResponse>, AppError> {
    let result = calculate_percentile(&payload.values, payload.percentile)?;

    Ok(Json(CalculateResponse {
        count: payload.values.len(),
        percentile: payload.percentile,
        result,
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
    let result = calculate_percentile(&values, percentile)?;

    Ok(Json(CalculateResponse {
        count: values.len(),
        percentile,
        result,
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
fn build_app() -> Router {
    Router::new()
        .route("/calculate", post(calculate))
        .route("/calculate/file", post(calculate_file))
        .route("/health", get(health))
        .merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
}

/// Start the API server
pub async fn serve(config: Config) -> anyhow::Result<()> {
    // Initialize tracing - keep guard alive for file logging
    let _guard = init_logging(&config)?;

    let app = build_app();

    let addr = SocketAddr::new(config.server.bind_ip, config.server.port);
    info!("🚀 Outlier API server listening on http://{}", addr);
    info!("📚 API documentation available at http://{}/docs", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    async fn response_json(response: Response) -> serde_json::Value {
        let body = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&body).unwrap()
    }

    // --- GET /health ---

    #[tokio::test]
    async fn health_returns_200() {
        let app = build_app();

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
        let app = build_app();

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
        let app = build_app();

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
        let app = build_app();

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
        let app = build_app();

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
        let app = build_app();

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
        let app = build_app();

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
        let app = build_app();
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
        let app = build_app();
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
        let app = build_app();
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
        let app = build_app();
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
        let app = build_app();
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
        let app = build_app();
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
        let app = build_app();
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
}
