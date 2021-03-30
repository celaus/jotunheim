use uuid::Uuid;
use anyhow::Result;
use crate::{CollectorAddr, msg::SetupMetrics};

#[cfg(feature="sensor-bme680")]
pub mod bme680;

#[cfg(feature="sensor-external")]
pub mod external;
