use uuid::Uuid;
use anyhow::Result;

use crate::{CollectorAddr, msg::SetupMetrics};

#[cfg(feature="switch-gpio")]
pub mod gpio;
