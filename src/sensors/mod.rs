use uuid::Uuid;
use anyhow::Result;
use crate::{CollectorAddr, msg::SetupMetrics};

#[cfg(feature="sensor-bme680")]
pub mod bme680;

#[cfg(feature="sensor-bmp180")]
pub mod bme680;

pub(crate) async fn setup_collectors(name: &str, collector: CollectorAddr) -> Result<Uuid> {
    let metrics = SetupMetrics::Gauge(
        name.to_owned(),
        vec![String::from("kind"), String::from("unit")],
    );
    collector.call(metrics).await?
}
