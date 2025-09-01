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
    data: RwLock<StatsData>,
    max_entries: usize,
}

#[derive(Debug)]
struct StatsData {
    first_response_times: Vec<f64>,
    total_response_times: Vec<f64>,
    quick_response_times: Vec<f64>,
    large_model_times: Vec<f64>,
    request_count: u64,
}

impl StatsCollector {
    pub fn new(max_entries: usize) -> Self {
        Self {
            data: RwLock::new(StatsData {
                first_response_times: Vec::new(),
                total_response_times: Vec::new(),
                quick_response_times: Vec::new(),
                large_model_times: Vec::new(),
                request_count: 0,
            }),
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
        let mut data = match self.data.write() {
            Ok(guard) => guard,
            Err(e) => {
                tracing::error!("StatsCollector lock poisoned: {}", e);
                // Return early to avoid panic
                return;
            }
        };

        // Add new data points
        data.first_response_times.push(first_response_time);
        data.total_response_times.push(total_time);

        if let Some(quick_time) = quick_time {
            data.quick_response_times.push(quick_time);
        }

        if let Some(large_time) = large_time {
            data.large_model_times.push(large_time);
        }

        data.request_count += 1;

        // Efficient memory management with reserve and batch removal
        if data.first_response_times.len() > self.max_entries {
            let keep_count = self.max_entries * 3 / 4; // Keep 75% to reduce frequent reallocations
            let remove_count = data.first_response_times.len() - keep_count;

            // Use efficient rotation to avoid multiple allocations
            data.first_response_times.drain(0..remove_count);
            data.total_response_times.drain(0..remove_count);

            // Handle optional data with different lengths
            if data.quick_response_times.len() > remove_count {
                data.quick_response_times.drain(0..remove_count);
            }
            if data.large_model_times.len() > remove_count {
                data.large_model_times.drain(0..remove_count);
            }

            // Pre-reserve space to avoid future reallocations
            data.first_response_times.reserve(self.max_entries / 4);
            data.total_response_times.reserve(self.max_entries / 4);
        }
    }

    pub fn get_stats(&self) -> serde_json::Value {
        let data = match self.data.read() {
            Ok(guard) => guard,
            Err(e) => {
                tracing::error!("StatsCollector lock poisoned during get_stats: {}", e);
                // Return default stats to avoid panic
                return serde_json::json!({
                    "total_requests": 0,
                    "first_response_latency": calculate_stats(&[]),
                    "total_response_latency": calculate_stats(&[]),
                    "quick_response_latency": calculate_stats(&[]),
                    "large_model_latency": calculate_stats(&[])
                });
            }
        };

        serde_json::json!({
            "total_requests": data.request_count,
            "first_response_latency": calculate_stats(&data.first_response_times),
            "total_response_latency": calculate_stats(&data.total_response_times),
            "quick_response_latency": calculate_stats(&data.quick_response_times),
            "large_model_latency": calculate_stats(&data.large_model_times)
        })
    }

    pub fn reset(&self) {
        let mut data = match self.data.write() {
            Ok(guard) => guard,
            Err(e) => {
                tracing::error!("StatsCollector lock poisoned during reset: {}", e);
                // Return early to avoid panic
                return;
            }
        };

        // Clear all data
        data.first_response_times.clear();
        data.total_response_times.clear();
        data.quick_response_times.clear();
        data.large_model_times.clear();
        data.request_count = 0;
    }

    pub fn get_request_count(&self) -> u64 {
        let data = match self.data.read() {
            Ok(guard) => guard,
            Err(e) => {
                tracing::error!(
                    "StatsCollector lock poisoned during get_request_count: {}",
                    e
                );
                // Return default count to avoid panic
                return 0;
            }
        };
        data.request_count
    }

    pub fn get_avg_first_response_time(&self) -> f64 {
        let data = match self.data.read() {
            Ok(guard) => guard,
            Err(e) => {
                tracing::error!(
                    "StatsCollector lock poisoned during get_avg_first_response_time: {}",
                    e
                );
                // Return default average to avoid panic
                return 0.0;
            }
        };
        if data.first_response_times.is_empty() {
            0.0
        } else {
            data.first_response_times.iter().sum::<f64>() / data.first_response_times.len() as f64
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
