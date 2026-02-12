use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

mod telemetry;

#[cfg(feature = "server")]
mod config;
#[cfg(feature = "server")]
mod server;

#[derive(Parser, Debug)]
#[command(name = "outlier")]
#[command(version)]
#[command(about = "Calculate percentiles from numerical datasets", long_about = None)]
struct Args {
    /// Start API server mode
    #[cfg(feature = "server")]
    #[arg(long)]
    serve: bool,

    /// Path to configuration file (TOML format)
    /// Can also be set via CONFIG_FILE environment variable
    #[cfg(feature = "server")]
    #[arg(short = 'c', long = "config")]
    config_file: Option<PathBuf>,

    /// Port for API server (only with --serve, overrides config file)
    #[cfg(feature = "server")]
    #[arg(long)]
    port: Option<u16>,

    /// Percentile to calculate (e.g., 95, 99)
    #[arg(short = 'p', long, default_value = "95")]
    percentile: f64,

    /// Input file (JSON or CSV format)
    #[arg(short = 'f', long)]
    file: Option<PathBuf>,

    /// Direct values from command line (comma-separated)
    #[arg(short = 'v', long, value_delimiter = ',')]
    values: Option<Vec<f64>>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Quick exit for --version/--help or no args - no telemetry initialization needed
    let args_count = std::env::args().count();
    if args_count == 1
        || std::env::args().any(|a| a == "-V" || a == "--version" || a == "-h" || a == "--help")
    {
        if args_count == 1 {
            // No arguments provided - show help
            Args::parse_from(["outlier", "--help"]);
        }
        Args::parse(); // Prints version/help and exits
    }

    let args = Args::parse();

    #[cfg(feature = "server")]
    if args.serve {
        // Load configuration
        let mut config = config::Config::load(args.config_file.as_ref())?;

        // CLI port overrides config file
        if let Some(port) = args.port {
            config.server.port = port;
        }

        // Start API server (server has its own logging via init_logging)
        return server::serve(config).await;
    }

    // Initialize Honeycomb telemetry only for CLI mode
    telemetry::init_telemetry();

    // Run CLI mode
    let result = run_cli(args);
    telemetry::shutdown_telemetry();
    result
}

#[tracing::instrument(skip_all, fields(percentile = %args.percentile))]
fn run_cli(args: Args) -> Result<()> {
    use outlier::{calculate_percentile, read_values_from_file};

    // Validate percentile
    if args.percentile < 0.0 || args.percentile > 100.0 {
        anyhow::bail!("Percentile must be between 0 and 100");
    }

    // Collect values from either file or CLI
    let values = if let Some(ref file_path) = args.file {
        read_values_from_file(file_path)?
    } else if let Some(values) = args.values {
        values
    } else {
        anyhow::bail!("Must provide either --file or --values");
    };

    if values.is_empty() {
        anyhow::bail!("No values provided");
    }

    // Calculate percentile
    let result = calculate_percentile(&values, args.percentile)?;

    println!("Number of values: {}", values.len());
    println!("Percentile (P{}): {:.2}", args.percentile, result);

    Ok(())
}
