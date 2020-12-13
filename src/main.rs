use anyhow::Result;
use async_std::prelude::*;
mod config;
mod db;
mod msg;
mod sensors;
use clap::{App as ClApp, Arg};

use config::Config;
use db::PrometheusCollector;
use env;
use log::info;
use msg::{EncodeData, SensorReading};
use sensors::bme680::{setup_collectors, Bme680SensorReader};
use std::{fs::File, future::Future, time::Duration};
use tide::{prelude::*, Response, StatusCode}; // Pulls in the json! macro.
use tide::{Body, Request};
use xactor::{Actor, Addr};
use envconfig::Envconfig;

pub(crate) type CollectorAddr = Addr<PrometheusCollector>;

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

pub(crate) fn server(addr: &str, collector: CollectorAddr) -> impl Future {
    let mut app = tide::with_state(AppState { collector });
    app.at("/metrics").get(metrics);
    app.listen(addr.to_owned())
}

#[async_std::main]
async fn main() -> Result<()> {
    let matches = ClApp::new("jotunheim")
        .version("0.1.0")
        .author("Claus Matzinger. <claus.matzinger+kb@gmail.com>")
        .about("A no-fluff Function-as-a-Service runtime for home use.")
        .get_matches();

    env_logger::init();
    let config: Config = Config::init_from_env()?;

    info!("Welcome to Jotunheim.");
    let addr = PrometheusCollector::new()?.start().await?;

    let id = setup_collectors(&config.metrics_name, addr.clone()).await?;

    let sensor = Bme680SensorReader::new("/dev/i2c-1", id, Duration::from_secs(1))?
        .start()
        .await?;
    let srv = server(&config.endpoint, addr.clone());
    srv.await;
    Ok(())
}
