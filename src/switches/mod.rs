use anyhow::Result;
use uuid::Uuid;

use crate::{msg::SetupMetrics, CollectorAddr};

#[cfg(feature = "switch-gpio")]
pub mod gpio;
