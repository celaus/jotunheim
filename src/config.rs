use anyhow::Result;
use std::io::Read;
use envconfig::Envconfig;



#[derive(Envconfig)]
pub struct Config {
    #[envconfig(from = "JH_ADDR", default = "0.0.0.0:7200")]
    pub endpoint: String,

    #[envconfig(from = "JH_NAME", default = "roomA")]
    pub metrics_name: String,
}

