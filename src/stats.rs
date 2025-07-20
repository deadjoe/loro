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

#[derive(Debug, Default)]
pub struct StatsCollector {
    first_response_times: RwLock<Vec<f64>>,
    total_response_times: RwLock<Vec<f64>>,
    quick_response_times: RwLock<Vec<f64>>,
    large_model_times: RwLock<Vec<f64>>,
    request_count: RwLock<u64>,
}

impl StatsCollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_request(
        &self,
        first_response_time: f64,
        total_time: f64,
        quick_time: Option<f64>,
        large_time: Option<f64>,
    ) {
        {
            let mut first_times = self.first_response_times.write().unwrap();
            first_times.push(first_response_time);
        }
        
        {
            let mut total_times = self.total_response_times.write().unwrap();
            total_times.push(total_time);
        }

        if let Some(quick_time) = quick_time {
            let mut quick_times = self.quick_response_times.write().unwrap();
            quick_times.push(quick_time);
        }

        if let Some(large_time) = large_time {
            let mut large_times = self.large_model_times.write().unwrap();
            large_times.push(large_time);
        }

        {
            let mut count = self.request_count.write().unwrap();
            *count += 1;
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
        {
            let mut first_times = self.first_response_times.write().unwrap();
            first_times.clear();
        }
        {
            let mut total_times = self.total_response_times.write().unwrap();
            total_times.clear();
        }
        {
            let mut quick_times = self.quick_response_times.write().unwrap();
            quick_times.clear();
        }
        {
            let mut large_times = self.large_model_times.write().unwrap();
            large_times.clear();
        }
        {
            let mut count = self.request_count.write().unwrap();
            *count = 0;
        }
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