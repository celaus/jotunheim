use std::collections::HashMap;

use crate::{
    msg::{EncodeData, SensorReading, SetupMetrics, Value},
    AccessoryType,
};
use async_std::task;
use dyn_fmt::AsStrFormatExt;
use log::{error, info};
use uuid::Uuid;
use xactor::*;

pub(crate) struct WebHookCollector {
    metrics: HashMap<Uuid, String>,
    url: String,
}

impl WebHookCollector {
    pub fn new<I: Into<String>>(url: I) -> Result<Self> {
        Ok(WebHookCollector {
            metrics: HashMap::new(),
            url: url.into(),
        })
    }
}

#[async_trait::async_trait]
impl Actor for WebHookCollector {
    async fn started(&mut self, ctx: &mut Context<Self>) -> Result<()> {
        ctx.subscribe::<SetupMetrics>().await?;
        ctx.subscribe::<SensorReading>().await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl Handler<SetupMetrics> for WebHookCollector {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: SetupMetrics) {
        info!("Setting up metrics: {:?}", msg);
        match msg {
            SetupMetrics::Gauge(id, name, labels) | SetupMetrics::Counter(id, name, labels) => {
                self.metrics.insert(id.clone(), name);
            }
        }
    }
}

#[async_trait::async_trait]
impl Handler<SensorReading> for WebHookCollector {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: SensorReading) {
        if let Some(accessory_id) = self.metrics.get(&msg.id) {
            info!("Reading for received: '{:?}'", msg);
            let s = match_state_str(accessory_id, msg.accessory_type, msg.reading);
            let webhook = self.url.format(&[&s]);
            task::spawn(async move {
                info!("Executing webhook URL: {}", webhook);
                info!(
                    "Response: {:?}",
                    reqwest::get(&webhook).await.unwrap().text().await.unwrap()
                );
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
            AccessoryType::Temperature => {
                "accessoryId={}-temperature&value={}".format(&[accessory_id, &val])
            }
            AccessoryType::Pressure => {
                "accessoryId={}-pressure&value={}".format(&[accessory_id, &val])
            }
            AccessoryType::Humidity => {
                "accessoryId={}-humidity&value={}".format(&[accessory_id, &val])
            }
            AccessoryType::GasResistance => {
                "accessoryId={}-gasresistance&value={}".format(&[accessory_id, &val])
            }
            AccessoryType::Switch => {
                let val = (val == "0.0").to_string();
                "accessoryId={}-switch&state={}".format(&[accessory_id, &val])
            }
            _ => "accessoryId={}&value={}".format(&[accessory_id, &val]),
        }
    } else {
        String::new()
    }
}
