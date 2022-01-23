use crate::{msg::SetupMetrics, CollectorAddr};
use anyhow::Result;
use uuid::Uuid;

#[cfg(feature = "sensor-bme680")]
pub mod bme680;

#[cfg(feature = "sensor-external")]
pub mod external;
