use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use tracing::instrument;

#[cfg(feature = "server")]
use utoipa::ToSchema;

/// Percentile interpolation method
#[cfg_attr(feature = "server", derive(ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
#[clap(rename_all = "snake_case")]
pub enum PercentileMethod {
    /// Linear interpolation between adjacent values (default)
    #[default]
    Linear,
    /// Round index to nearest integer
    NearestRank,
    /// Always round index down
    Lower,
    /// Always round index up
    Upper,
    /// Average of floor and ceil values
    Midpoint,
    /// Round half to even index (banker's rounding)
    NearestEven,
}

impl fmt::Display for PercentileMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PercentileMethod::Linear => write!(f, "linear"),
            PercentileMethod::NearestRank => write!(f, "nearest_rank"),
            PercentileMethod::Lower => write!(f, "lower"),
            PercentileMethod::Upper => write!(f, "upper"),
            PercentileMethod::Midpoint => write!(f, "midpoint"),
            PercentileMethod::NearestEven => write!(f, "nearest_even"),
        }
    }
}

/// CSV record structure for parsing
#[derive(Debug, Deserialize)]
pub struct ValueRecord {
    pub value: f64,
}

/// Request structure for calculate API endpoint
#[cfg_attr(feature = "server", derive(ToSchema))]
#[derive(Debug, Deserialize, Serialize)]
pub struct CalculateRequest {
    /// Array of numerical values
    pub values: Vec<f64>,
    /// Percentile to calculate (0-100)
    #[serde(default = "default_percentile")]
    pub percentile: f64,
    /// Interpolation method (defaults to linear)
    #[serde(default)]
    pub method: PercentileMethod,
}

fn default_percentile() -> f64 {
    95.0
}

/// Response structure for calculate API endpoint
#[cfg_attr(feature = "server", derive(ToSchema))]
#[derive(Debug, Serialize, Deserialize)]
pub struct CalculateResponse {
    /// Number of values in the dataset
    pub count: usize,
    /// The requested percentile value
    pub percentile: f64,
    /// The calculated result
    pub result: f64,
    /// The interpolation method used
    #[serde(default)]
    pub method: PercentileMethod,
}

/// Error response structure
#[cfg_attr(feature = "server", derive(ToSchema))]
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    /// Error message
    pub error: String,
}

/// Calculate percentile from a slice of values
///
/// Values are sorted internally, so the input order doesn't matter.
/// The `method` parameter selects the interpolation algorithm.
///
/// # Arguments
/// * `values` - Slice of f64 values
/// * `percentile` - Percentile to calculate (0-100)
/// * `method` - Interpolation method
///
/// # Returns
/// * `Ok(f64)` - The calculated percentile value
/// * `Err` - If values is empty or percentile is out of range
///
/// # Examples
/// ```
/// use outlier::{calculate_percentile, PercentileMethod};
///
/// let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
/// let p50 = calculate_percentile(&values, 50.0, PercentileMethod::Linear).unwrap();
/// assert_eq!(p50, 3.0);
/// ```
#[instrument(skip(values), fields(value_count = values.len(), percentile = %percentile, method = %method))]
pub fn calculate_percentile(
    values: &[f64],
    percentile: f64,
    method: PercentileMethod,
) -> Result<f64> {
    if values.is_empty() {
        anyhow::bail!("Cannot calculate percentile of empty dataset");
    }

    if !(0.0..=100.0).contains(&percentile) {
        anyhow::bail!("Percentile must be between 0 and 100");
    }

    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let index = (percentile / 100.0) * (sorted.len() - 1) as f64;
    let lower = index.floor() as usize;
    let upper = index.ceil() as usize;

    match method {
        PercentileMethod::Linear => {
            if lower == upper {
                Ok(sorted[lower])
            } else {
                let weight = index - lower as f64;
                Ok(sorted[lower] * (1.0 - weight) + sorted[upper] * weight)
            }
        }
        PercentileMethod::NearestRank => Ok(sorted[index.round() as usize]),
        PercentileMethod::Lower => Ok(sorted[lower]),
        PercentileMethod::Upper => Ok(sorted[upper]),
        PercentileMethod::Midpoint => Ok((sorted[lower] + sorted[upper]) / 2.0),
        PercentileMethod::NearestEven => {
            let rounded = bankers_round(index) as usize;
            Ok(sorted[rounded])
        }
    }
}

/// Banker's rounding: round half to even
fn bankers_round(value: f64) -> f64 {
    let rounded = value.round();
    let diff = (value - value.floor() - 0.5).abs();
    if diff < f64::EPSILON {
        // Exactly halfway — round to even
        if (rounded as u64).is_multiple_of(2) {
            rounded
        } else {
            rounded - 1.0
        }
    } else {
        rounded
    }
}

/// Read values from a file (JSON or CSV format)
#[instrument(fields(path = %path.display()))]
pub fn read_values_from_file(path: &Path) -> Result<Vec<f64>> {
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .context("Unable to determine file extension")?;

    match extension.to_lowercase().as_str() {
        "json" => read_json_file(path),
        "csv" => read_csv_file(path),
        _ => anyhow::bail!("Unsupported file format. Use .json or .csv"),
    }
}

/// Read values from a JSON file (expects array of numbers)
pub fn read_json_file(path: &Path) -> Result<Vec<f64>> {
    let file = File::open(path).context("Failed to open JSON file")?;
    let reader = BufReader::new(file);
    let values: Vec<f64> = serde_json::from_reader(reader)
        .context("Failed to parse JSON file. Expected array of numbers.")?;

    const MAX_VALUES: usize = 10_000_000; // 10 million
    if values.len() > MAX_VALUES {
        anyhow::bail!(
            "Input dataset exceeds the limit of {} values. Aborting.",
            MAX_VALUES
        );
    }

    Ok(values)
}

/// Read values from a CSV file (expects header row "value")
pub fn read_csv_file(path: &Path) -> Result<Vec<f64>> {
    let file = File::open(path).context("Failed to open CSV file")?;
    let mut reader = csv::Reader::from_reader(file);
    let mut values = Vec::new();
    const MAX_VALUES: usize = 10_000_000; // 10 million

    for result in reader.deserialize() {
        if values.len() >= MAX_VALUES {
            anyhow::bail!(
                "Input dataset exceeds the limit of {} values. Aborting.",
                MAX_VALUES
            );
        }
        let record: ValueRecord = result.context("Failed to parse CSV record")?;
        values.push(record.value);
    }

    Ok(values)
}

/// Parse values from bytes (JSON or CSV)
#[instrument(skip(bytes), fields(filename = %filename, byte_count = bytes.len()))]
pub fn read_values_from_bytes(bytes: &[u8], filename: &str) -> Result<Vec<f64>> {
    let extension = filename.split('.').next_back().unwrap_or("");

    match extension.to_lowercase().as_str() {
        "json" => {
            let values: Vec<f64> = serde_json::from_slice(bytes)
                .context("Failed to parse JSON. Expected array of numbers.")?;
            const MAX_VALUES: usize = 10_000_000; // 10 million
            if values.len() > MAX_VALUES {
                anyhow::bail!(
                    "Input dataset exceeds the limit of {} values. Aborting.",
                    MAX_VALUES
                );
            }
            Ok(values)
        }
        "csv" => {
            let mut reader = csv::Reader::from_reader(bytes);
            let mut values = Vec::new();
            const MAX_VALUES: usize = 10_000_000; // 10 million

            for result in reader.deserialize() {
                if values.len() >= MAX_VALUES {
                    anyhow::bail!(
                        "Input dataset exceeds the limit of {} values. Aborting.",
                        MAX_VALUES
                    );
                }
                let record: ValueRecord = result.context("Failed to parse CSV record")?;
                values.push(record.value);
            }

            Ok(values)
        }
        _ => anyhow::bail!("Unsupported file format. Use .json or .csv"),
    }
}

#[cfg(test)]
mod tests;
