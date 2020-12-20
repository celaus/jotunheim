use uuid::Uuid;
use anyhow::Result;

use crate::{CollectorAddr, msg::SetupMetrics};

#[cfg(feature="switch-gpio")]
pub mod gpio;

pub(crate) async fn setup_collectors(name: &str, collector: CollectorAddr) -> Result<Uuid> {
    let metrics = SetupMetrics::Gauge(
        name.to_owned(),
        vec![String::from("name")],
    );
    collector.call(metrics).await?
}
