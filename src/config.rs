use envconfig::Envconfig;

#[derive(Envconfig)]
pub struct Config {
    #[envconfig(from = "JH_ADDR", default = "0.0.0.0:7200")]
    pub endpoint: String,

    #[envconfig(from = "JH_NAME", default = "roomA")]
    pub metrics_name: String,

    #[envconfig(from = "JH_GPIOS")]
    pub gpios: Option<String>,

    #[envconfig(from = "JH_WEBHOOK")]
    pub webhook: Option<String>,
}

pub async fn parse_gpios(tuples: &str) -> Vec<(String, u32)> {
    tuples
        .split(',')
        .filter_map(|t| t.find(':').map(|p| t.split_at(p)))
        .filter_map(|(a, b)| {
            if let Some(v) = b[1..].parse::<u32>().ok() {
                Some((a.to_string(), v))
            } else {
                None
            }
        })
        .collect()
}
