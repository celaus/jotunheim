use crate::msg::{Switch, SwitchState, Value};
use crate::switches::SetupMetrics;
use log::info;
use rust_gpiozero::*;
use uuid::Uuid;
use xactor::*;

use anyhow::Result;

use crate::msg::{ReadNow, SensorReading};

pub(crate) struct GpioSwitch {
    dev: DigitalOutputDevice,
    state: bool,
    collector_id: Uuid,
    name: String,
    pin_no: u32,
}

impl GpioSwitch {
    pub fn new<I: Into<String>>(pin_no: u32, name: I) -> Self {
        let dev = DigitalOutputDevice::new(pin_no as u8);
        let collector_id = Uuid::new_v4();
        GpioSwitch {
            dev,
            state: false, // major assumption :)
            collector_id,
            name: name.into(),
            pin_no,
        }
    }
}

#[async_trait::async_trait]
impl Actor for GpioSwitch {
    async fn started(&mut self, _ctx: &mut Context<Self>) -> anyhow::Result<()> {
        let mut addr = Broker::from_registry().await?;
        addr.publish(SetupMetrics::Gauge(
            self.collector_id,
            format!("switch:{}", self.name),
            vec![String::from("name")],
        ))?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl Handler<ReadNow> for GpioSwitch {
    async fn handle(&mut self, _ctx: &mut Context<Self>, _msg: ReadNow) {
        let mut addr = Broker::from_registry().await.unwrap();

        let _ = addr.publish(SensorReading {
            id: self.collector_id,
            reading: Value::Simple(if self.state { 1.0 } else { 0.0 }),
            labels: vec![self.name.clone()],
        });
    }
}

#[async_trait::async_trait]
impl Handler<SwitchState> for GpioSwitch {
    async fn handle(&mut self, _ctx: &mut Context<Self>, _msg: SwitchState) -> bool {
        self.state
    }
}

#[async_trait::async_trait]
impl Handler<Switch> for GpioSwitch {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: Switch) -> Result<()> {
        info!("Setting GPIO '{}' to {:?}", self.name, msg);
        let _value = match msg {
            Switch::On => self.dev.on(),
            Switch::Off => self.dev.off(),
        };
        Ok(())
    }
}
