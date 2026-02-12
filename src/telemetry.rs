use opentelemetry::trace::TracerProvider;
use opentelemetry::{KeyValue, StringValue};
use opentelemetry_otlp::{Protocol, WithExportConfig, WithTonicConfig};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::trace::SdkTracerProvider;
use std::sync::OnceLock;
use tonic::transport::ClientTlsConfig;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const HONEYCOMB_ENDPOINT: &str = "https://api.honeycomb.io:443";

/// Global storage for the tracer provider so we can shut it down later.
static TRACER_PROVIDER: OnceLock<SdkTracerProvider> = OnceLock::new();

/// Initialize telemetry with Honeycomb via OpenTelemetry.
///
/// If `HONEYCOMB_API_KEY` is set, traces are exported to Honeycomb.
/// Otherwise, only console logging is enabled.
pub fn init_telemetry() {
    let api_key = std::env::var("HONEYCOMB_API_KEY").ok();
    let service_name = std::env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| "outlier".to_string());

    // Create the base subscriber with fmt layer for console output
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .compact();

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    if let Some(api_key) = api_key {
        // Configure OTLP exporter for Honeycomb with TLS
        let tls_config = ClientTlsConfig::new().with_native_roots();

        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_protocol(Protocol::Grpc)
            .with_endpoint(HONEYCOMB_ENDPOINT)
            .with_tls_config(tls_config)
            .with_metadata({
                let mut metadata = tonic::metadata::MetadataMap::new();
                metadata.insert(
                    "x-honeycomb-team",
                    api_key.parse().expect("Invalid API key format"),
                );
                metadata
            })
            .build()
            .expect("Failed to create OTLP exporter");

        let resource = Resource::builder()
            .with_attributes(vec![KeyValue::new(
                "service.name",
                StringValue::from(service_name),
            )])
            .build();

        let tracer_provider = SdkTracerProvider::builder()
            .with_batch_exporter(exporter)
            .with_resource(resource)
            .build();

        let tracer = tracer_provider.tracer("outlier");

        // Store provider for later shutdown
        let _ = TRACER_PROVIDER.set(tracer_provider);

        let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .with(otel_layer)
            .init();

        tracing::info!("Honeycomb telemetry initialized");
    } else {
        // No API key - just use console logging
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .init();

        tracing::debug!("Honeycomb API key not set, using console logging only");
    }
}

/// Shutdown the telemetry pipeline, flushing any pending spans.
pub fn shutdown_telemetry() {
    if let Some(provider) = TRACER_PROVIDER.get() {
        let _ = provider.shutdown();
    }
}
