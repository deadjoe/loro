use serde::{Deserialize, Serialize};
use std::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyStats {
    pub avg: f64,
    pub min: f64,
    pub max: f64,
    pub p50: f64,
    pub p95: f64,
}

#[derive(Debug)]
pub struct StatsCollector {
    first_response_times: RwLock<Vec<f64>>,
    total_response_times: RwLock<Vec<f64>>,
    quick_response_times: RwLock<Vec<f64>>,
    large_model_times: RwLock<Vec<f64>>,
    request_count: RwLock<u64>,
    max_entries: usize,
}

impl StatsCollector {
    pub fn new(max_entries: usize) -> Self {
        Self {
            first_response_times: RwLock::new(Vec::new()),
            total_response_times: RwLock::new(Vec::new()),
            quick_response_times: RwLock::new(Vec::new()),
            large_model_times: RwLock::new(Vec::new()),
            request_count: RwLock::new(0),
            max_entries,
        }
    }

    pub fn add_request(
        &self,
        first_response_time: f64,
        total_time: f64,
        quick_time: Option<f64>,
        large_time: Option<f64>,
    ) {
        // Safely acquire all locks in a consistent order to prevent deadlocks
        // Use expect() with clear error messages instead of unwrap()
        let mut first_times = self.first_response_times.write()
            .expect("Failed to acquire write lock for first_response_times");
        let mut total_times = self.total_response_times.write()
            .expect("Failed to acquire write lock for total_response_times");
        let mut quick_times = self.quick_response_times.write()
            .expect("Failed to acquire write lock for quick_response_times");
        let mut large_times = self.large_model_times.write()
            .expect("Failed to acquire write lock for large_model_times");
        let mut count = self.request_count.write()
            .expect("Failed to acquire write lock for request_count");

        // Add new data points
        first_times.push(first_response_time);
        total_times.push(total_time);

        if let Some(quick_time) = quick_time {
            quick_times.push(quick_time);
        }

        if let Some(large_time) = large_time {
            large_times.push(large_time);
        }

        *count += 1;

        // Efficient memory management with reserve and batch removal
        if first_times.len() > self.max_entries {
            let keep_count = self.max_entries * 3 / 4; // Keep 75% to reduce frequent reallocations
            let remove_count = first_times.len() - keep_count;
            
            // Use efficient rotation to avoid multiple allocations
            first_times.drain(0..remove_count);
            total_times.drain(0..remove_count);

            // Handle optional data with different lengths
            if quick_times.len() > remove_count {
                quick_times.drain(0..remove_count);
            }
            if large_times.len() > remove_count {
                large_times.drain(0..remove_count);
            }
            
            // Pre-reserve space to avoid future reallocations
            first_times.reserve(self.max_entries / 4);
            total_times.reserve(self.max_entries / 4);
        }
    }

    pub fn get_stats(&self) -> serde_json::Value {
        // Safely acquire read locks with descriptive error messages
        let first_times = self.first_response_times.read()
            .expect("Failed to acquire read lock for first_response_times");
        let total_times = self.total_response_times.read()
            .expect("Failed to acquire read lock for total_response_times");
        let quick_times = self.quick_response_times.read()
            .expect("Failed to acquire read lock for quick_response_times");
        let large_times = self.large_model_times.read()
            .expect("Failed to acquire read lock for large_model_times");
        let count = *self.request_count.read()
            .expect("Failed to acquire read lock for request_count");

        serde_json::json!({
            "total_requests": count,
            "first_response_latency": calculate_stats(&first_times),
            "total_response_latency": calculate_stats(&total_times),
            "quick_response_latency": calculate_stats(&quick_times),
            "large_model_latency": calculate_stats(&large_times)
        })
    }

    pub fn reset(&self) {
        // Safely acquire all locks in consistent order to prevent deadlocks
        let mut first_times = self.first_response_times.write()
            .expect("Failed to acquire write lock for first_response_times during reset");
        let mut total_times = self.total_response_times.write()
            .expect("Failed to acquire write lock for total_response_times during reset");
        let mut quick_times = self.quick_response_times.write()
            .expect("Failed to acquire write lock for quick_response_times during reset");
        let mut large_times = self.large_model_times.write()
            .expect("Failed to acquire write lock for large_model_times during reset");
        let mut count = self.request_count.write()
            .expect("Failed to acquire write lock for request_count during reset");

        // Clear all data
        first_times.clear();
        total_times.clear();
        quick_times.clear();
        large_times.clear();
        *count = 0;
    }

    pub fn get_request_count(&self) -> u64 {
        *self.request_count.read()
            .expect("Failed to acquire read lock for request_count")
    }

    pub fn get_avg_first_response_time(&self) -> f64 {
        let times = self.first_response_times.read()
            .expect("Failed to acquire read lock for first_response_times");
        if times.is_empty() {
            0.0
        } else {
            times.iter().sum::<f64>() / times.len() as f64
        }
    }
}

pub fn calculate_stats(data: &[f64]) -> LatencyStats {
    if data.is_empty() {
        return LatencyStats {
            avg: 0.0,
            min: 0.0,
            max: 0.0,
            p50: 0.0,
            p95: 0.0,
        };
    }

    // Filter out NaN values and sort
    let filtered_data: Vec<f64> = data.iter().filter(|&&x| x.is_finite()).copied().collect();
    if filtered_data.is_empty() {
        return LatencyStats {
            avg: 0.0,
            min: 0.0,
            max: 0.0,
            p50: 0.0,
            p95: 0.0,
        };
    }
    
    let mut sorted_data = filtered_data;
    sorted_data.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    // Double-check: sorted_data should not be empty after filtering
    if sorted_data.is_empty() {
        return LatencyStats {
            avg: 0.0,
            min: 0.0,
            max: 0.0,
            p50: 0.0,
            p95: 0.0,
        };
    }
    
    let avg = sorted_data.iter().sum::<f64>() / sorted_data.len() as f64;
    let min = sorted_data[0];
    let max = sorted_data[sorted_data.len() - 1];

    // Use proper percentile calculation (0-based indexing)
    let p50_idx = if sorted_data.len() == 1 {
        0
    } else {
        ((sorted_data.len() - 1) as f64 * 0.5) as usize
    };
    let p50 = sorted_data[p50_idx];

    let p95_idx = if sorted_data.len() == 1 {
        0
    } else {
        ((sorted_data.len() - 1) as f64 * 0.95) as usize
    };
    let p95 = sorted_data[p95_idx];

    LatencyStats {
        avg,
        min,
        max,
        p50,
        p95,
    }
}
