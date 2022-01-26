use std::{collections::HashMap, time::Duration};

use envconfig::Envconfig;

#[derive(Envconfig)]
pub struct Config {
    #[envconfig(from = "JH_ADDR", default = "0.0.0.0:7200")]
    pub endpoint: String,

    #[envconfig(from = "JH_NAME", default = "roomA")]
    pub metrics_name: String,

    #[envconfig(from = "JH_GPIOS")]
    pub gpios: Option<String>,

    #[envconfig(from = "JH_EXTERNALS")]
    pub externals: Option<String>,

    #[envconfig(from = "JH_BME680", default = "/dev/i2c-1")]
    pub bme680: String,

    #[envconfig(from = "JH_WEBHOOK")]
    pub webhook: Option<String>,

    #[envconfig(from = "JH_RESOLUTION_MS")]
    pub resolution_ms: u64,

    #[envconfig(from = "JH_API_CREDENTIALS")]
    pub api_credentials: String,
}

impl Config {
    pub async fn parsed_gpios(&self) -> Vec<(String, u32)> {
        match &self.gpios {
            Some(tuples) => tuples
                .split(',')
                .filter_map(|t| t.find(':').map(|p| t.split_at(p)))
                .filter_map(|(a, b)| {
                    if let Some(v) = b[1..].parse::<u32>().ok() {
                        Some((a.to_string(), v))
                    } else {
                        None
                    }
                })
                .collect(),
            _ => {
                vec![]
            }
        }
    }

    pub async fn parsed_externals(&self) -> Vec<String> {
        match &self.externals {
            Some(tuples) => tuples.split(',').map(|e| e.trim().to_string()).collect(),
            _ => {
                vec![]
            }
        }
    }

    pub async fn parsed_credentials(&self) -> HashMap<String, String> {
        let tuples: &Vec<_> = &self
            .api_credentials
            .split(',')
            .map(|e| e.trim().to_string())
            .collect();
        (&tuples).iter().chunks_exact(2).collect()
    }

    pub fn resolution(&self) -> Duration {
        Duration::from_millis(self.resolution_ms)
    }
}
