use anyhow::Result;
mod config;
mod db;
mod msg;
mod sensors;
mod utils;

#[cfg(feature = "switch-gpio")]
mod switches;
mod webhook;
use clap::App as ClApp;

use config::Config;
use db::PrometheusCollector;

use log::info;
use msg::EncodeData;

#[cfg(feature = "sensor-bme680")]
use sensors::bme680::Bme680SensorReader;

use envconfig::Envconfig;
use std::time::Duration;
use tide::{Body, Request};
use tide::{Response, StatusCode}; // Pulls in the json! macro.
use webhook::WebHookCollector;
use xactor::{Actor, Addr};

pub(crate) type CollectorAddr = Addr<PrometheusCollector>;
use serde::{Deserialize, Serialize};

#[cfg(feature = "sensor-external")]
use crate::sensors::external;
use crate::utils::e_;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
enum AccessoryType {
    Temperature,
    Pressure,
    Humidity,
    GasResistance,
    Switch,
    Co2,
    Wind,
    Rain,
    Unknown,
}

impl Default for AccessoryType {
    fn default() -> Self {
        AccessoryType::Unknown
    }
}

#[derive(Clone)]
pub struct AppState {
    collector: CollectorAddr,
}

async fn metrics(req: Request<AppState>) -> tide::Result {
    let state = req.state();
    let data = state.collector.call(EncodeData).await??;
    let mut resp = Response::new(StatusCode::Ok);
    resp.set_body(Body::from_string(data));
    Ok(resp)
}

#[async_std::main]
async fn main() -> Result<()> {
    let _ = ClApp::new("jotunheim")
        .version("0.1.0")
        .author("Claus Matzinger. <claus.matzinger+kb@gmail.com>")
        .about("A no-fluff sensor reader.")
        .get_matches();

    env_logger::init();
    let config: Config = Config::init_from_env()?;

    info!("Welcome to Jotunheim.");
    let prometheus = PrometheusCollector::new()?.start().await?;
    let _webhooks = if let Some(webhook_url) = &config.webhook {
        info!("Found webhook URL: {}", webhook_url);
        Some(WebHookCollector::new(webhook_url)?.start().await?)
    } else {
        None
    };

    #[cfg(feature = "sensor-external")]
    let _external_actors = external::setup(&config).await?;

    #[cfg(feature = "sensor-bme680")]
    let _bme = sensors::bme680::setup(&config).await;

    #[cfg(feature = "sensor-api")]
    let _netatmo = sensors::api::netatmo::setup(&config).await;

    let mut app = tide::with_state(AppState {
        collector: prometheus,
    });
    app.at("/metrics").get(metrics);

    #[cfg(feature = "switch-gpio")]
    app.at("/s")
        .nest(switches::http_handlers::init_and_setup(&config).await?);

    let addr = config.endpoint;
    info!("Serving at {}", addr);
    app.listen(addr.to_owned()).await.map_err(e_)
}
