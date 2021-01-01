use std::collections::HashMap;

use crate::{
    msg::{EncodeData, SensorReading, SetupMetrics, Value},
    AccessoryType,
};
use async_std::task;
use dyn_fmt::AsStrFormatExt;
use log::error;
use uuid::Uuid;
use xactor::*;

pub(crate) struct WebHooker {
    metrics: HashMap<Uuid, String>,
    url: String,
}

impl WebHooker {
    pub fn new<I: Into<String>>(url: I) -> Result<Self> {
        Ok(WebHooker {
            metrics: HashMap::new(),
            url: url.into(),
        })
    }
}

#[async_trait::async_trait]
impl Actor for WebHooker {
    async fn started(&mut self, ctx: &mut Context<Self>) -> Result<()> {
        ctx.subscribe::<SetupMetrics>().await?;
        ctx.subscribe::<SensorReading>().await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl Handler<SetupMetrics> for WebHooker {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: SetupMetrics) {
        match msg {
            SetupMetrics::Gauge(id, name, labels) | SetupMetrics::Counter(id, name, labels) => {
                self.metrics.insert(id.clone(), name);
            }
        }
    }
}

#[async_trait::async_trait]
impl Handler<SensorReading> for WebHooker {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: SensorReading) {
        if let Some(accessory_id) = self.metrics.get(&msg.id) {
            let s = match_state_str(accessory_id, msg.accessory_type, msg.reading);
            let webhook = self.url.format(&[&s]);
            task::spawn(async move {
                ureq::get(&webhook);
            })
            .await;
        } else {
            error!("Couldn't find a webhook '{}'", msg.id);
        }
    }
}

fn match_state_str(accessory_id: &str, t: AccessoryType, val: Value) -> String {
    if let Value::Simple(val) = val {
        let val = val.to_string();
        match t {
            AccessoryType::Temperature
            | AccessoryType::Pressure
            | AccessoryType::Humidity
            | AccessoryType::GasResistance => {
                "accessoryId={}&value={}".format(&[accessory_id, &val])
            }
            AccessoryType::Switch => {
                let val = (val == "0.0").to_string();
                "accessoryId={}&state={}".format(&[accessory_id, &val])
            }
            _ => "accessoryId={}&value={}".format(&[accessory_id, &val]),
        }
    } else {
        String::new()
    }
}
