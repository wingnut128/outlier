use super::*;

// ========================
// Existing tests (updated to pass PercentileMethod::Linear)
// ========================

#[test]
fn test_calculate_percentile_simple() {
    let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let result = calculate_percentile(&values, 50.0, PercentileMethod::Linear).unwrap();
    assert_eq!(result, 3.0);
}

#[test]
fn test_calculate_percentile_95th() {
    let values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
    let result = calculate_percentile(&values, 95.0, PercentileMethod::Linear).unwrap();
    assert!((result - 9.55).abs() < 0.01);
}

#[test]
fn test_calculate_percentile_99th() {
    let values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
    let result = calculate_percentile(&values, 99.0, PercentileMethod::Linear).unwrap();
    assert!((result - 9.91).abs() < 0.01);
}

#[test]
fn test_calculate_percentile_0th() {
    let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let result = calculate_percentile(&values, 0.0, PercentileMethod::Linear).unwrap();
    assert_eq!(result, 1.0);
}

#[test]
fn test_calculate_percentile_100th() {
    let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let result = calculate_percentile(&values, 100.0, PercentileMethod::Linear).unwrap();
    assert_eq!(result, 5.0);
}

#[test]
fn test_calculate_percentile_empty() {
    let values: Vec<f64> = vec![];
    let result = calculate_percentile(&values, 50.0, PercentileMethod::Linear);
    assert!(result.is_err());
}

#[test]
fn test_calculate_percentile_single_value() {
    let values = vec![42.0];
    let result = calculate_percentile(&values, 50.0, PercentileMethod::Linear).unwrap();
    assert_eq!(result, 42.0);
}

#[test]
fn test_calculate_percentile_unsorted_input() {
    let values = vec![5.0, 1.0, 3.0, 2.0, 4.0];
    let result = calculate_percentile(&values, 50.0, PercentileMethod::Linear).unwrap();
    assert_eq!(result, 3.0);
}

#[test]
fn test_calculate_percentile_with_duplicates() {
    let values = vec![1.0, 2.0, 2.0, 3.0, 4.0];
    let result = calculate_percentile(&values, 50.0, PercentileMethod::Linear).unwrap();
    assert_eq!(result, 2.0);
}

#[test]
fn test_calculate_percentile_large_dataset() {
    let values: Vec<f64> = (1..=1000).map(|x| x as f64).collect();
    let result = calculate_percentile(&values, 95.0, PercentileMethod::Linear).unwrap();
    assert!((result - 950.05).abs() < 0.01);
}

#[test]
fn test_percentile_out_of_range() {
    let values = vec![1.0, 2.0, 3.0];
    assert!(calculate_percentile(&values, -1.0, PercentileMethod::Linear).is_err());
    assert!(calculate_percentile(&values, 101.0, PercentileMethod::Linear).is_err());
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

// ========================
// Per-algorithm tests at P40 on [1,2,3,4,5]
// index = (40/100) * 4 = 1.6
// sorted = [1,2,3,4,5], floor=1, ceil=2
// ========================

#[test]
fn test_method_linear_p40() {
    let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    // linear: sorted[1]*(1-0.6) + sorted[2]*0.6 = 2*0.4 + 3*0.6 = 0.8+1.8 = 2.6
    let result = calculate_percentile(&values, 40.0, PercentileMethod::Linear).unwrap();
    assert!((result - 2.6).abs() < 1e-10);
}

#[test]
fn test_method_nearest_rank_p40() {
    let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    // round(1.6) = 2, sorted[2] = 3.0
    let result = calculate_percentile(&values, 40.0, PercentileMethod::NearestRank).unwrap();
    assert_eq!(result, 3.0);
}

#[test]
fn test_method_lower_p40() {
    let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    // floor(1.6) = 1, sorted[1] = 2.0
    let result = calculate_percentile(&values, 40.0, PercentileMethod::Lower).unwrap();
    assert_eq!(result, 2.0);
}

#[test]
fn test_method_upper_p40() {
    let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    // ceil(1.6) = 2, sorted[2] = 3.0
    let result = calculate_percentile(&values, 40.0, PercentileMethod::Upper).unwrap();
    assert_eq!(result, 3.0);
}

#[test]
fn test_method_midpoint_p40() {
    let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    // (sorted[1] + sorted[2]) / 2 = (2+3)/2 = 2.5
    let result = calculate_percentile(&values, 40.0, PercentileMethod::Midpoint).unwrap();
    assert!((result - 2.5).abs() < 1e-10);
}

#[test]
fn test_method_nearest_even_p40() {
    let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    // index=1.6, not halfway → normal round → 2, sorted[2] = 3.0
    let result = calculate_percentile(&values, 40.0, PercentileMethod::NearestEven).unwrap();
    assert_eq!(result, 3.0);
}

// Test nearest_even at an exact halfway point
// [1,2,3,4,5] P50: index = 0.5*4 = 2.0 → exact, all methods agree
// Use [1,2,3,4,5,6,7,8,9] P50: index = 0.5*8 = 4.0 → exact
// For half index, try P62.5 on [1,2,3,4,5]: index = 0.625*4 = 2.5 (halfway)
#[test]
fn test_method_nearest_even_halfway() {
    let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    // P62.5: index = 0.625 * 4 = 2.5 (exactly halfway between 2 and 3)
    // Banker's round: 2.5 → nearest even = 2, sorted[2] = 3.0
    let result = calculate_percentile(&values, 62.5, PercentileMethod::NearestEven).unwrap();
    assert_eq!(result, 3.0);
}

#[test]
fn test_method_nearest_even_halfway_odd() {
    let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    // P37.5: index = 0.375 * 4 = 1.5 (exactly halfway between 1 and 2)
    // Banker's round: 1.5 → nearest even = 2, sorted[2] = 3.0
    let result = calculate_percentile(&values, 37.5, PercentileMethod::NearestEven).unwrap();
    assert_eq!(result, 3.0);
}

// ========================
// Boundary tests (P0, P100, single value) across all methods
// ========================

#[test]
fn test_all_methods_p0() {
    let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let methods = [
        PercentileMethod::Linear,
        PercentileMethod::NearestRank,
        PercentileMethod::Lower,
        PercentileMethod::Upper,
        PercentileMethod::Midpoint,
        PercentileMethod::NearestEven,
    ];
    for method in methods {
        let result = calculate_percentile(&values, 0.0, method).unwrap();
        assert_eq!(result, 1.0, "P0 failed for method {:?}", method);
    }
}

#[test]
fn test_all_methods_p100() {
    let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let methods = [
        PercentileMethod::Linear,
        PercentileMethod::NearestRank,
        PercentileMethod::Lower,
        PercentileMethod::Upper,
        PercentileMethod::Midpoint,
        PercentileMethod::NearestEven,
    ];
    for method in methods {
        let result = calculate_percentile(&values, 100.0, method).unwrap();
        assert_eq!(result, 5.0, "P100 failed for method {:?}", method);
    }
}

#[test]
fn test_all_methods_single_value() {
    let values = vec![42.0];
    let methods = [
        PercentileMethod::Linear,
        PercentileMethod::NearestRank,
        PercentileMethod::Lower,
        PercentileMethod::Upper,
        PercentileMethod::Midpoint,
        PercentileMethod::NearestEven,
    ];
    for method in methods {
        let result = calculate_percentile(&values, 50.0, method).unwrap();
        assert_eq!(result, 42.0, "Single value failed for method {:?}", method);
    }
}

// ========================
// Serde tests
// ========================

#[test]
fn test_percentile_method_serde_roundtrip() {
    let methods = [
        PercentileMethod::Linear,
        PercentileMethod::NearestRank,
        PercentileMethod::Lower,
        PercentileMethod::Upper,
        PercentileMethod::Midpoint,
        PercentileMethod::NearestEven,
    ];
    for method in methods {
        let json = serde_json::to_string(&method).unwrap();
        let deserialized: PercentileMethod = serde_json::from_str(&json).unwrap();
        assert_eq!(method, deserialized);
    }
}

#[test]
fn test_percentile_method_serde_snake_case() {
    assert_eq!(
        serde_json::to_string(&PercentileMethod::NearestRank).unwrap(),
        "\"nearest_rank\""
    );
    assert_eq!(
        serde_json::to_string(&PercentileMethod::NearestEven).unwrap(),
        "\"nearest_even\""
    );
    assert_eq!(
        serde_json::to_string(&PercentileMethod::Linear).unwrap(),
        "\"linear\""
    );
}

#[test]
fn test_calculate_request_default_method() {
    let json = r#"{"values": [1.0, 2.0], "percentile": 50.0}"#;
    let req: CalculateRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.method, PercentileMethod::Linear);
}

#[test]
fn test_calculate_response_default_method() {
    let json = r#"{"count": 5, "percentile": 50.0, "result": 3.0}"#;
    let resp: CalculateResponse = serde_json::from_str(json).unwrap();
    assert_eq!(resp.method, PercentileMethod::Linear);
}

#[test]
fn test_percentile_method_display() {
    assert_eq!(PercentileMethod::Linear.to_string(), "linear");
    assert_eq!(PercentileMethod::NearestRank.to_string(), "nearest_rank");
    assert_eq!(PercentileMethod::Lower.to_string(), "lower");
    assert_eq!(PercentileMethod::Upper.to_string(), "upper");
    assert_eq!(PercentileMethod::Midpoint.to_string(), "midpoint");
    assert_eq!(PercentileMethod::NearestEven.to_string(), "nearest_even");
}
