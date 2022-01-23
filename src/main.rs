use anyhow::Result;
mod config;
mod db;
mod msg;
mod sensors;
mod switches;
mod webhook;
use clap::App as ClApp;

use config::Config;
use db::PrometheusCollector;

use log::info;
use msg::{EncodeData, Switch, SwitchState};

#[cfg(feature = "sensor-bme680")]
use sensors::{bme680::Bme680SensorReader, external::ExternalSensorReader};

#[cfg(feature = "switch-gpio")]
use switches::gpio::GpioSwitch;

use envconfig::Envconfig;
use std::{collections::HashMap, future::Future, time::Duration};
use tide::{Body, Request};
use tide::{Response, StatusCode}; // Pulls in the json! macro.
use webhook::WebHookCollector;
use xactor::{Actor, Addr};

pub(crate) type CollectorAddr = Addr<PrometheusCollector>;
pub(crate) type SwitchAddr = Addr<GpioSwitch>;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
enum AccessoryType {
    Temperature,
    Pressure,
    Humidity,
    GasResistance,
    Switch,
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
    gpio: HashMap<String, SwitchAddr>,
}

async fn switch(req: Request<AppState>) -> tide::Result {
    let id = req.param("id")?;
    let on_off: i32 = req.param("value")?.parse()?;

    let state = req.state();
    let mut resp = Response::new(StatusCode::Ok);

    if let Some(gpio) = state.gpio.get(id) {
        info!("Triggering GPIO '{}'", id);
        if on_off == 0 {
            let _ = gpio.call(Switch::Off).await?;
        } else {
            let _ = gpio.call(Switch::On).await?;
        }
    } else {
        resp = Response::new(StatusCode::NotFound);
    }
    Ok(resp)
}

async fn switch_status(req: Request<AppState>) -> tide::Result {
    let id = req.param("id")?;

    let state = req.state();
    let mut resp = Response::new(StatusCode::Ok);

    if let Some(gpio) = state.gpio.get(id) {
        let result = gpio.call(SwitchState {}).await? as u8;
        resp.set_body(Body::from_string(format!("{}", result)));
    } else {
        resp = Response::new(StatusCode::NotFound);
    }
    Ok(resp)
}

async fn metrics(req: Request<AppState>) -> tide::Result {
    let state = req.state();
    let data = state.collector.call(EncodeData).await??;
    let mut resp = Response::new(StatusCode::Ok);
    resp.set_body(Body::from_string(data));
    Ok(resp)
}

pub(crate) fn server(
    addr: &str,
    collector: CollectorAddr,
    switches: HashMap<String, SwitchAddr>,
) -> impl Future {
    let mut app = tide::with_state(AppState {
        collector,
        gpio: switches,
    });
    app.at("/metrics").get(metrics);
    app.at("/s/:id/").get(switch_status);
    app.at("/s/:id/:value").get(switch);
    app.listen(addr.to_owned())
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
    let addr = PrometheusCollector::new()?.start().await?;
    let _webhooks = if let Some(webhook_url) = &config.webhook {
        info!("Found webhook URL: {}", webhook_url);
        Some(WebHookCollector::new(webhook_url)?.start().await?)
    } else {
        None
    };

    let mut switches = HashMap::new();
    let mut external_actors = vec![];
    let gpios = config.parsed_gpios().await;

    #[cfg(feature = "switch-gpio")]
    if !gpios.is_empty() {
        info!("GPIO module active");
        let switches_actors: HashMap<String, GpioSwitch> = gpios
            .into_iter()
            .map(|(n, p)| (n.clone(), GpioSwitch::new(p, n)))
            .collect();
        info!(
            "GPIOs activated: {:?}",
            switches_actors.keys().collect::<Vec<&String>>()
        );
        for (name, actor) in switches_actors {
            let a = actor.start().await?;
            switches.insert(name, a);
        }
    }

    let externals = config.parsed_externals().await;

    #[cfg(feature = "sensor-external")]
    if !externals.is_empty() {
        info!(
            "External Sensor module active, {} paths found",
            externals.len()
        );
        for actor in externals
            .into_iter()
            .map(|p| ExternalSensorReader::new(p, vec![], Duration::from_secs(1)))
        {
            let a = actor.start().await?;
            external_actors.push(a);
        }
    }

    #[cfg(feature = "sensor-bme680")]
    let _bme = if sensors::bme680::is_available("/dev/i2c-1") {
        Bme680SensorReader::new("/dev/i2c-1", config.metrics_name, Duration::from_secs(1))?
            .start()
            .await
            .ok()
    } else {
        None
    };

    let srv = server(&config.endpoint, addr.clone(), switches);
    info!("Everything set up and good to go.");
    srv.await;
    Ok(())
}
