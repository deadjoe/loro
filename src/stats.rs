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
        // Acquire all locks in a consistent order to prevent deadlocks
        let mut first_times = self.first_response_times.write().unwrap();
        let mut total_times = self.total_response_times.write().unwrap();
        let mut quick_times = self.quick_response_times.write().unwrap();
        let mut large_times = self.large_model_times.write().unwrap();
        let mut count = self.request_count.write().unwrap();

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

        // Prevent memory leaks by limiting the number of stored entries
        if first_times.len() > self.max_entries {
            let remove_count = first_times.len() - self.max_entries;
            first_times.drain(0..remove_count);
            total_times.drain(0..remove_count);

            // Handle optional data with different lengths
            if quick_times.len() > remove_count {
                quick_times.drain(0..remove_count);
            }
            if large_times.len() > remove_count {
                large_times.drain(0..remove_count);
            }
        }
    }

    pub fn get_stats(&self) -> serde_json::Value {
        let first_times = self.first_response_times.read().unwrap();
        let total_times = self.total_response_times.read().unwrap();
        let quick_times = self.quick_response_times.read().unwrap();
        let large_times = self.large_model_times.read().unwrap();
        let count = *self.request_count.read().unwrap();

        serde_json::json!({
            "total_requests": count,
            "first_response_latency": calculate_stats(&first_times),
            "total_response_latency": calculate_stats(&total_times),
            "quick_response_latency": calculate_stats(&quick_times),
            "large_model_latency": calculate_stats(&large_times)
        })
    }

    pub fn reset(&self) {
        // Acquire all locks in consistent order to prevent deadlocks
        let mut first_times = self.first_response_times.write().unwrap();
        let mut total_times = self.total_response_times.write().unwrap();
        let mut quick_times = self.quick_response_times.write().unwrap();
        let mut large_times = self.large_model_times.write().unwrap();
        let mut count = self.request_count.write().unwrap();

        // Clear all data
        first_times.clear();
        total_times.clear();
        quick_times.clear();
        large_times.clear();
        *count = 0;
    }

    pub fn get_request_count(&self) -> u64 {
        *self.request_count.read().unwrap()
    }

    pub fn get_avg_first_response_time(&self) -> f64 {
        let times = self.first_response_times.read().unwrap();
        if times.is_empty() {
            0.0
        } else {
            times.iter().sum::<f64>() / times.len() as f64
        }
    }
}

fn calculate_stats(data: &[f64]) -> LatencyStats {
    if data.is_empty() {
        return LatencyStats {
            avg: 0.0,
            min: 0.0,
            max: 0.0,
            p50: 0.0,
            p95: 0.0,
        };
    }

    let mut sorted_data = data.to_vec();
    sorted_data.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let avg = data.iter().sum::<f64>() / data.len() as f64;
    let min = sorted_data[0];
    let max = sorted_data[sorted_data.len() - 1];

    let p50_idx = (sorted_data.len() as f64 * 0.5) as usize;
    let p50 = if p50_idx < sorted_data.len() {
        sorted_data[p50_idx]
    } else {
        sorted_data[sorted_data.len() - 1]
    };

    let p95_idx = (sorted_data.len() as f64 * 0.95) as usize;
    let p95 = if p95_idx < sorted_data.len() {
        sorted_data[p95_idx]
    } else {
        sorted_data[sorted_data.len() - 1]
    };

    LatencyStats {
        avg,
        min,
        max,
        p50,
        p95,
    }
}
