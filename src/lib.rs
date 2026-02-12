use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use tracing::instrument;

#[cfg(feature = "server")]
use utoipa::ToSchema;

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
/// Uses linear interpolation for accurate percentile calculation.
/// Values are sorted internally, so the input order doesn't matter.
///
/// # Arguments
/// * `values` - Slice of f64 values
/// * `percentile` - Percentile to calculate (0-100)
///
/// # Returns
/// * `Ok(f64)` - The calculated percentile value
/// * `Err` - If values is empty or percentile is out of range
///
/// # Examples
/// ```
/// use outlier::calculate_percentile;
///
/// let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
/// let p50 = calculate_percentile(&values, 50.0).unwrap();
/// assert_eq!(p50, 3.0);
/// ```
#[instrument(skip(values), fields(value_count = values.len(), percentile = %percentile))]
pub fn calculate_percentile(values: &[f64], percentile: f64) -> Result<f64> {
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

    if lower == upper {
        Ok(sorted[lower])
    } else {
        let weight = index - lower as f64;
        Ok(sorted[lower] * (1.0 - weight) + sorted[upper] * weight)
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
