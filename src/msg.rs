use xactor::*;

#[message]
#[derive(Clone, Debug)]
pub(crate) struct SensorReading {
    pub id: uuid::Uuid,
    pub reading: Value,
    pub labels: Vec<String>
}

#[derive(Clone, Debug)]
pub(crate) enum Value {
    Simple(f32),
    Inc,
    Dec,
}

#[message(result = "anyhow::Result<uuid::Uuid>")]
pub(crate) enum SetupMetrics {
    Gauge(String, Vec<String>),
    Counter(String, Vec<String>),
}

#[message]
pub(crate) struct ReadNow;

#[message(result = "anyhow::Result<String>")]
pub(crate) struct EncodeData;
