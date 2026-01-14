use anyhow::{Context, Result};
use clap::Parser;
use serde::Deserialize;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "prate")]
#[command(version)]
#[command(about = "Calculate percentiles from numerical datasets", long_about = None)]
struct Args {
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

#[derive(Debug, Deserialize)]
struct ValueRecord {
    value: f64,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Validate percentile
    if args.percentile < 0.0 || args.percentile > 100.0 {
        anyhow::bail!("Percentile must be between 0 and 100");
    }

    // Collect values from either file or CLI
    let values = if let Some(file_path) = args.file {
        read_values_from_file(&file_path)?
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

fn read_values_from_file(path: &PathBuf) -> Result<Vec<f64>> {
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

fn read_json_file(path: &PathBuf) -> Result<Vec<f64>> {
    let file = File::open(path).context("Failed to open JSON file")?;
    let reader = BufReader::new(file);
    let values: Vec<f64> = serde_json::from_reader(reader)
        .context("Failed to parse JSON file. Expected array of numbers.")?;
    Ok(values)
}

fn read_csv_file(path: &PathBuf) -> Result<Vec<f64>> {
    let file = File::open(path).context("Failed to open CSV file")?;
    let mut reader = csv::Reader::from_reader(file);
    let mut values = Vec::new();

    for result in reader.deserialize() {
        let record: ValueRecord = result.context("Failed to parse CSV record")?;
        values.push(record.value);
    }

    Ok(values)
}

fn calculate_percentile(values: &[f64], percentile: f64) -> Result<f64> {
    if values.is_empty() {
        anyhow::bail!("Cannot calculate percentile of empty dataset");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_percentile_simple() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let result = calculate_percentile(&values, 50.0).unwrap();
        assert_eq!(result, 3.0);
    }

    #[test]
    fn test_calculate_percentile_95th() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        let result = calculate_percentile(&values, 95.0).unwrap();
        assert!((result - 9.55).abs() < 0.01);
    }

    #[test]
    fn test_calculate_percentile_99th() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        let result = calculate_percentile(&values, 99.0).unwrap();
        assert!((result - 9.91).abs() < 0.01);
    }

    #[test]
    fn test_calculate_percentile_0th() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let result = calculate_percentile(&values, 0.0).unwrap();
        assert_eq!(result, 1.0);
    }

    #[test]
    fn test_calculate_percentile_100th() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let result = calculate_percentile(&values, 100.0).unwrap();
        assert_eq!(result, 5.0);
    }

    #[test]
    fn test_calculate_percentile_empty() {
        let values: Vec<f64> = vec![];
        let result = calculate_percentile(&values, 50.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_calculate_percentile_single_value() {
        let values = vec![42.0];
        let result = calculate_percentile(&values, 50.0).unwrap();
        assert_eq!(result, 42.0);
    }

    #[test]
    fn test_calculate_percentile_unsorted_input() {
        let values = vec![5.0, 1.0, 3.0, 2.0, 4.0];
        let result = calculate_percentile(&values, 50.0).unwrap();
        assert_eq!(result, 3.0);
    }

    #[test]
    fn test_calculate_percentile_with_duplicates() {
        let values = vec![1.0, 2.0, 2.0, 3.0, 4.0];
        let result = calculate_percentile(&values, 50.0).unwrap();
        assert_eq!(result, 2.0);
    }

    #[test]
    fn test_calculate_percentile_large_dataset() {
        let values: Vec<f64> = (1..=1000).map(|x| x as f64).collect();
        let result = calculate_percentile(&values, 95.0).unwrap();
        assert!((result - 950.05).abs() < 0.01);
    }
}
