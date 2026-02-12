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

#[test]
fn test_percentile_out_of_range() {
    let values = vec![1.0, 2.0, 3.0];
    assert!(calculate_percentile(&values, -1.0).is_err());
    assert!(calculate_percentile(&values, 101.0).is_err());
}

#[test]
#[ignore]
fn test_read_csv_from_bytes_limit_exceeded() {
    let mut csv_data = String::from(
        "value
",
    );
    // Create a CSV with 10,000,001 values, which is one over the limit.
    for i in 0..=10_000_000 {
        csv_data.push_str(&format!(
            "{}
",
            i
        ));
    }

    let result = read_values_from_bytes(csv_data.as_bytes(), "data.csv");
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Input dataset exceeds the limit of 10000000 values. Aborting.")
    );
}
