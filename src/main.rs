use anyhow::Result;
mod config;
mod db;
mod msg;
mod sensors;
mod utils;

#[cfg(feature = "switch-gpio")]
mod switches;
use clap::App as ClApp;

use config::Config;
use db::PrometheusCollector;

use log::info;
use msg::EncodeData;

use envconfig::Envconfig;
use tide::{Body, Request};
use tide::{Response, StatusCode}; // Pulls in the json! macro.
use xactor::{Actor, Addr};

pub(crate) type CollectorAddr = Addr<PrometheusCollector>;

#[cfg(feature = "sensor-external")]
use crate::sensors::external;
use crate::utils::e_;

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

    #[cfg(feature = "sensor-external")]
    let _external_actors = external::setup(&config).await?;

    #[cfg(feature = "sensor-bme680")]
    let _bme = sensors::bme680::setup(&config).await?;

    #[cfg(feature = "sensor-api")]
    let _netatmo = sensors::api::netatmo::setup(&config).await?;

    let mut app = tide::with_state(AppState {
        collector: prometheus,
    });
    app.at("/metrics").get(metrics);

    #[cfg(feature = "switch-gpio")]
    app.at("/s")
        .nest(switches::http_handlers::init_and_setup(&config).await?);
    println!("####");
    let addr = config.endpoint;
    info!("Serving at {}", addr);
    app.listen(addr.to_owned()).await.map_err(e_)
}
