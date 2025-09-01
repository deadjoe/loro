use loro::stats::{calculate_stats, LatencyStats, StatsCollector};
use std::sync::Arc;
use tokio::task;

#[test]
fn test_stats_collector_basic_functionality() {
    let collector = StatsCollector::new(100);
    
    // Test initial state
    assert_eq!(collector.get_request_count(), 0);
    assert_eq!(collector.get_avg_first_response_time(), 0.0);
    
    // Add some data
    collector.add_request(1.0, 2.0, Some(0.5), Some(1.5));
    assert_eq!(collector.get_request_count(), 1);
    assert_eq!(collector.get_avg_first_response_time(), 1.0);
    
    // Add more data
    collector.add_request(3.0, 4.0, Some(1.5), Some(2.5));
    assert_eq!(collector.get_request_count(), 2);
    assert_eq!(collector.get_avg_first_response_time(), 2.0); // (1.0 + 3.0) / 2
}

#[test]
fn test_stats_collector_memory_management() {
    let collector = StatsCollector::new(5); // Small limit for testing
    
    // Add more entries than the limit
    for i in 0..10 {
        collector.add_request(
            i as f64,
            (i * 2) as f64,
            Some((i as f64) * 0.5),
            Some((i as f64) * 1.5),
        );
    }
    
    // Should have limited the stored entries
    let stats = collector.get_stats();
    assert!(stats["total_requests"].as_u64().unwrap() == 10);
    
    // Internal arrays should be limited in size (not directly testable, but memory should be managed)
}

#[tokio::test]
async fn test_stats_collector_concurrent_access() {
    let collector = Arc::new(StatsCollector::new(1000));
    let mut handles = vec![];
    
    // Spawn multiple tasks that add data concurrently
    for i in 0..100 {
        let collector_clone = Arc::clone(&collector);
        let handle = task::spawn(async move {
            collector_clone.add_request(
                i as f64,
                (i * 2) as f64,
                Some((i as f64) * 0.5),
                Some((i as f64) * 1.5),
            );
        });
        handles.push(handle);
    }
    
    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }
    
    // Verify final count
    assert_eq!(collector.get_request_count(), 100);
}

#[tokio::test]
async fn test_stats_collector_concurrent_read_write() {
    let collector = Arc::new(StatsCollector::new(1000));
    let mut handles = vec![];
    
    // Spawn writers
    for i in 0..50 {
        let collector_clone = Arc::clone(&collector);
        let handle = task::spawn(async move {
            collector_clone.add_request(
                i as f64,
                (i * 2) as f64,
                Some((i as f64) * 0.5),
                Some((i as f64) * 1.5),
            );
        });
        handles.push(handle);
    }
    
    // Spawn readers
    for _ in 0..50 {
        let collector_clone = Arc::clone(&collector);
        let handle = task::spawn(async move {
            let _stats = collector_clone.get_stats();
            let _count = collector_clone.get_request_count();
            let _avg = collector_clone.get_avg_first_response_time();
        });
        handles.push(handle);
    }
    
    // Wait for all operations to complete
    for handle in handles {
        handle.await.unwrap();
    }
    
    // Should complete without deadlocks or panics
    assert_eq!(collector.get_request_count(), 50);
}

#[test]
fn test_stats_collector_reset_functionality() {
    let collector = StatsCollector::new(100);
    
    // Add some data
    collector.add_request(1.0, 2.0, Some(0.5), Some(1.5));
    collector.add_request(3.0, 4.0, Some(1.5), Some(2.5));
    
    assert_eq!(collector.get_request_count(), 2);
    assert_ne!(collector.get_avg_first_response_time(), 0.0);
    
    // Reset
    collector.reset();
    
    assert_eq!(collector.get_request_count(), 0);
    assert_eq!(collector.get_avg_first_response_time(), 0.0);
}

#[test]
fn test_calculate_stats_empty_data() {
    let stats = calculate_stats(&[]);
    
    assert_eq!(stats.avg, 0.0);
    assert_eq!(stats.min, 0.0);
    assert_eq!(stats.max, 0.0);
    assert_eq!(stats.p50, 0.0);
    assert_eq!(stats.p95, 0.0);
}

#[test]
fn test_calculate_stats_single_value() {
    let stats = calculate_stats(&[5.0]);
    
    assert_eq!(stats.avg, 5.0);
    assert_eq!(stats.min, 5.0);
    assert_eq!(stats.max, 5.0);
    assert_eq!(stats.p50, 5.0);
    assert_eq!(stats.p95, 5.0);
}

#[test]
fn test_calculate_stats_multiple_values() {
    let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let stats = calculate_stats(&data);
    
    assert_eq!(stats.avg, 3.0);
    assert_eq!(stats.min, 1.0);
    assert_eq!(stats.max, 5.0);
    assert_eq!(stats.p50, 3.0); // Middle value
    assert_eq!(stats.p95, 5.0); // 95th percentile
}

#[test]
fn test_calculate_stats_with_nan_values() {
    let data = vec![1.0, f64::NAN, 3.0, f64::INFINITY, 5.0, f64::NEG_INFINITY];
    let stats = calculate_stats(&data);
    
    // Should filter out non-finite values and calculate from [1.0, 3.0, 5.0]
    assert_eq!(stats.avg, 3.0);
    assert_eq!(stats.min, 1.0);
    assert_eq!(stats.max, 5.0);
}

#[test]
fn test_calculate_stats_all_nan() {
    let data = vec![f64::NAN, f64::INFINITY, f64::NEG_INFINITY];
    let stats = calculate_stats(&data);
    
    // Should return default stats when all values are non-finite
    assert_eq!(stats.avg, 0.0);
    assert_eq!(stats.min, 0.0);
    assert_eq!(stats.max, 0.0);
    assert_eq!(stats.p50, 0.0);
    assert_eq!(stats.p95, 0.0);
}

#[test]
fn test_calculate_stats_percentiles() {
    // Test with 100 values to verify percentile calculations
    let data: Vec<f64> = (1..=100).map(|i| i as f64).collect();
    let stats = calculate_stats(&data);
    
    assert_eq!(stats.avg, 50.5);
    assert_eq!(stats.min, 1.0);
    assert_eq!(stats.max, 100.0);
    
    // P50 should be around the 50th percentile (middle)
    assert!((stats.p50 - 50.0).abs() < 1.0);
    
    // P95 should be around the 95th percentile
    assert!((stats.p95 - 95.0).abs() < 1.0);
}

#[test]
fn test_latency_stats_struct() {
    let stats = LatencyStats {
        avg: 1.5,
        min: 1.0,
        max: 2.0,
        p50: 1.4,
        p95: 1.9,
    };
    
    // Test that the struct can be serialized/deserialized
    let json = serde_json::to_string(&stats).unwrap();
    let deserialized: LatencyStats = serde_json::from_str(&json).unwrap();
    
    assert_eq!(stats.avg, deserialized.avg);
    assert_eq!(stats.min, deserialized.min);
    assert_eq!(stats.max, deserialized.max);
    assert_eq!(stats.p50, deserialized.p50);
    assert_eq!(stats.p95, deserialized.p95);
}

#[test]
fn test_stats_collector_optional_data() {
    let collector = StatsCollector::new(100);
    
    // Test with only required data (no quick/large times)
    collector.add_request(1.0, 2.0, None, None);
    
    let stats = collector.get_stats();
    assert_eq!(stats["total_requests"], 1);
    
    // Quick and large response stats should still be present but empty
    let quick_stats = &stats["quick_response_latency"];
    let large_stats = &stats["large_model_latency"];
    
    assert_eq!(quick_stats["avg"], 0.0);
    assert_eq!(large_stats["avg"], 0.0);
}

#[test]
fn test_stats_collector_mixed_optional_data() {
    let collector = StatsCollector::new(100);
    
    // Add mixed data - some with optional times, some without
    collector.add_request(1.0, 2.0, Some(0.5), Some(1.5));
    collector.add_request(3.0, 4.0, None, None);
    collector.add_request(5.0, 6.0, Some(2.5), Some(4.5));
    
    let stats = collector.get_stats();
    assert_eq!(stats["total_requests"], 3);
    
    // Should calculate averages correctly for the data that was provided
    let quick_stats = &stats["quick_response_latency"];
    let large_stats = &stats["large_model_latency"];
    
    // Should have stats for the 2 entries that provided optional data
    assert_ne!(quick_stats["avg"], 0.0);
    assert_ne!(large_stats["avg"], 0.0);
}

#[test]
fn test_stats_collector_edge_case_sizes() {
    // Test with very small max_entries
    let small_collector = StatsCollector::new(1);
    small_collector.add_request(1.0, 2.0, Some(0.5), Some(1.5));
    small_collector.add_request(3.0, 4.0, Some(1.5), Some(2.5));
    
    // Should handle gracefully
    assert!(small_collector.get_request_count() >= 1);
    
    // Test with zero max_entries (edge case)
    let zero_collector = StatsCollector::new(0);
    zero_collector.add_request(1.0, 2.0, Some(0.5), Some(1.5));
    
    // Should not crash
    let _stats = zero_collector.get_stats();
}