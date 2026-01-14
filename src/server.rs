use axum::{
    Json, Router,
    extract::Multipart,
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
async fn health() -> Json<serde_json::Value> {
    Json(json!({
        "status": "healthy",
        "service": "outlier",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// Start the API server
pub async fn serve(port: u16) -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_target(false)
        .compact()
        .init();

    // Create router with all endpoints
    let app = Router::new()
        .route("/calculate", post(calculate))
        .route("/calculate/file", post(calculate_file))
        .route("/health", get(health))
        .merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("🚀 Outlier API server listening on http://{}", addr);
    info!("📚 API documentation available at http://{}/docs", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
