use xactor::*;

use crate::AccessoryType;

#[message]
#[derive(Clone, Debug)]
pub(crate) struct SensorReading {
    pub id: uuid::Uuid,
    pub reading: Value,
    pub labels: Vec<String>,
    pub accessory_type: AccessoryType,
}

#[derive(Clone, Debug)]
pub(crate) enum Value {
    Simple(f32),
    Inc,
    Dec,
}

#[message]
#[derive(Clone, Debug)]
pub(crate) enum SetupMetrics {
    Gauge(uuid::Uuid, String, Vec<String>),
    Counter(uuid::Uuid, String, Vec<String>),
}

#[message]
#[derive(Clone, Debug)]

pub(crate) struct ReadNow;


#[message(result = "bool")]
pub(crate) struct SwitchState;

#[message(result = "anyhow::Result<String>")]
pub(crate) struct EncodeData;

#[message(result = "anyhow::Result<()>")]
#[derive(Debug)]
pub(crate) enum Switch {
    On,
    Off
}
