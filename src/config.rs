use anyhow::Result;
use envconfig::Envconfig;
use std::{collections::HashMap, time::Duration};

#[derive(Envconfig, Default)]
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

    #[envconfig(from = "JH_RESOLUTION_MS", default = "1000")]
    pub resolution_ms: u64,

    #[envconfig(from = "JH_API_CREDENTIALS")]
    pub api_credentials: String,

    #[envconfig(from = "JH_LOCATION")]
    pub location: String,
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
        self.api_credentials
            .split(",")
            .map(|e| e.trim().to_string())
            .fold(HashMap::new(), |mut h, s| {
                if let Some(n) = s.find(':') {
                    let (k, v) = s.split_at(n + 1);
                    let decoded = base64::decode(v.trim()).unwrap();
                    let v = String::from_utf8_lossy(&decoded);
                    h.insert(k.trim().trim_end_matches(':').to_string(), v.to_string());
                }
                h
            })
    }

    pub fn location_rect(&self) -> Result<((f64, f64), (f64, f64))> {
        let rect = geohash::decode_bbox(&self.location)?;
        Ok((rect.min().x_y(), rect.max().x_y()))
    }

    pub fn resolution(&self) -> Duration {
        Duration::from_millis(self.resolution_ms)
    }
}

#[cfg(test)]
mod tests {
    #![allow(non_snake_case)]
    use super::*;

    #[async_std::test]
    async fn test_Config_parse_credentials_is_base64() {
        let mut conf = Config::default();
        let (v1, v2) = ("id|user|password|clientid|secret", "abcd");
        conf.api_credentials = format!(
            "netatmo:{},someotherservice:{}",
            base64::encode(v1),
            base64::encode(v2)
        );
        let expected = {
            let mut h = HashMap::new();
            h.insert("netatmo".to_string(), v1.to_string());
            h.insert("someotherservice".to_string(), v2.to_string());
            h
        };
        assert_eq!(conf.parsed_credentials().await, expected);
    }
}
