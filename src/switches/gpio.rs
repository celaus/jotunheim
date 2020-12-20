use crate::msg::{Switch, SwitchState, Value};
use rust_gpiozero::*;
use log::{debug, info};
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
    pub fn new(pin_no: u32, collector_id: Uuid, name: String) -> Self {
        let mut dev = DigitalOutputDevice::new(pin_no as u8);

        GpioSwitch {
            dev,
            state: false, // major assumption :)
            collector_id,
            name,
            pin_no
        }
    }
}

impl Actor for GpioSwitch {}

#[async_trait::async_trait]
impl Handler<ReadNow> for GpioSwitch {
    async fn handle(&mut self, _ctx: &mut Context<Self>, _msg: ReadNow) {
        let mut addr = Broker::from_registry().await.unwrap();

        addr.publish(SensorReading {
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
        let value = match msg {
            Switch::On => self.dev.on(),
            Switch::Off => self.dev.off(),
        };
        info!("Setting GPIO '{}' to {:?}", self.name, msg);
        
        Ok(())
    }
}
