use anyhow::Result;
use async_std::prelude::*;
mod config;
mod db;
mod msg;
mod sensors;
mod switches;
mod webhook;
use clap::{App as ClApp, Arg};

use config::{parse_gpios, Config};
use db::PrometheusCollector;
use env;
use log::info;
use msg::{EncodeData, SensorReading, Switch, SwitchState};

#[cfg(feature = "sensor-bme680")]
use sensors::bme680::{setup_collectors, Bme680SensorReader};

#[cfg(feature = "switch-gpio")]
use switches::gpio::GpioSwitch;

use envconfig::Envconfig;
use webhook::WebHooker;
use std::{collections::HashMap, fs::File, future::Future, time::Duration};
use tide::{prelude::*, Response, StatusCode}; // Pulls in the json! macro.
use tide::{Body, Request};
use uuid::Uuid;
use xactor::{Actor, Addr};

pub(crate) type CollectorAddr = Addr<PrometheusCollector>;
pub(crate) type SwitchAddr = Addr<GpioSwitch>;

#[derive(Debug, Clone, Copy)]
enum AccessoryType {
    Temperature,
    Pressure,
    Humidity,
    GasResistance,
    Switch,
    Unknown,
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
            gpio.call(Switch::Off).await?;
        } else {
            gpio.call(Switch::On).await?;
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
    app.at("/s/:id/:value").get(switch);
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
    if let Some(webhook_url) = &config.webhook {
        let _ = WebHooker::new(webhook_url)?.start().await?;
    }

    let mut switches = HashMap::new();

    #[cfg(feature = "switch-gpio")]
    if let Some(s) = config.gpios {
        info!("GPIO module active");
        let gpios = parse_gpios(&s).await;
        info!("... parsing {:?}", gpios);
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

    #[cfg(feature = "sensor-bme680")]
    let _ = Bme680SensorReader::new("/dev/i2c-1", config.metrics_name, Duration::from_secs(1))?
        .start()
        .await?;
    let srv = server(&config.endpoint, addr.clone(), switches);
    info!("Everything set up and good to go.");
    srv.await;
    Ok(())
}
