use std::collections::HashMap;

use anyhow::bail;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use super::requests::HeaterFanStateUpdateRequest;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HeaterFanMode {
    Normal,
    Natural,
    Sleep,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HeatStatus {
    Idle,
    Active,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum HeaterFanState {
    PowerOn(bool),
    Mode(HeaterFanMode),
    TargetTemperature(u8),
    CurrentTemperature(u8), //
    FanSpeed(u8),           // 1-10
    Oscillate(bool),
    Timer(u16),
    Silent(bool),
    Heater(bool),
    VentHeat(bool),
    HeatStatus(HeatStatus),
    Error(u8), // bitmap
}

impl HeaterFanState {
    pub fn parse_by_key(k: &str, v: JsonValue) -> anyhow::Result<Self> {
        let p = k.rfind('/').unwrap();
        match k.split_at(p + 1) {
            (_, "power_on") => Ok(HeaterFanState::PowerOn(serde_json::from_value(v)?)),
            (_, "mode") => Ok(HeaterFanState::Mode(serde_json::from_value(v)?)),
            (_, "target_temperature") => Ok(HeaterFanState::TargetTemperature(
                serde_json::from_value(v)?,
            )),
            (_, "current_temperature") => Ok(HeaterFanState::CurrentTemperature(
                serde_json::from_value(v)?,
            )),
            (_, "fan_speed") => Ok(HeaterFanState::FanSpeed(serde_json::from_value(v)?)),
            (_, "oscillate") => Ok(HeaterFanState::Oscillate(serde_json::from_value(v)?)),
            (_, "timer") => Ok(HeaterFanState::Timer(serde_json::from_value(v)?)),
            (_, "silent") => Ok(HeaterFanState::Silent(serde_json::from_value(v)?)),
            (_, "heater") => Ok(HeaterFanState::Heater(serde_json::from_value(v)?)),
            (_, "vent_heat") => Ok(HeaterFanState::VentHeat(serde_json::from_value(v)?)),
            (_, "heat_status") => Ok(HeaterFanState::HeatStatus(serde_json::from_value(v)?)),
            (_, "error") => Ok(HeaterFanState::Error(serde_json::from_value(v)?)),
            _ => bail!("Invalid key: {}", k),
        }
    }
}

pub enum HeaterFan {
    Properties,
    Update,
    WifiLed,
    Online,
    Version,
    Token,
    Name,
    _Type,
    Model,
}

// format!("appliance/heaterfan/{}/$properties", device_id),
// format!("appliance/heaterfan/{}/$update", device_id),
// format!("appliance/heaterfan/{}/$wifi_led", device_id),
// format!("appliance/heaterfan/{}/$online", device_id),
// format!("appliance/heaterfan/{}/$version", device_id),
// format!("appliance/heaterfan/{}/$token", device_id),
// format!("appliance/heaterfan/{}/$name", device_id),
// format!("appliance/heaterfan/{}/$type", device_id),
// format!("appliance/heaterfan/{}/$model", device_id),

pub fn to_topic(device_id: &str, function: &str) -> String {
    format!("appliance/heaterfan/{}/state/{}", device_id, function)
}

pub fn operation_state(device_id: &str) -> HashMap<String, HeaterFanState> {
    vec![
        (
            to_topic(device_id, "power_on"),
            HeaterFanState::PowerOn(false),
        ),
        (
            to_topic(device_id, "mode"),
            HeaterFanState::Mode(HeaterFanMode::Normal),
        ),
        (
            to_topic(device_id, "target_temperature"),
            HeaterFanState::TargetTemperature(0),
        ),
        (
            format!(
                "appliance/heaterfan/{}/state/current_temperature",
                device_id
            ),
            HeaterFanState::CurrentTemperature(0),
        ),
        (
            to_topic(device_id, "fan_speed"),
            HeaterFanState::FanSpeed(0),
        ),
        (
            to_topic(device_id, "oscillate"),
            HeaterFanState::Oscillate(false),
        ),
        (to_topic(device_id, "timer"), HeaterFanState::Timer(0)),
        (to_topic(device_id, "silent"), HeaterFanState::Silent(false)),
        (to_topic(device_id, "heater"), HeaterFanState::Heater(false)),
        (
            to_topic(device_id, "vent_heat"),
            HeaterFanState::VentHeat(false),
        ),
        (
            to_topic(device_id, "heat_status"),
            HeaterFanState::HeatStatus(HeatStatus::Idle),
        ),
        (to_topic(device_id, "error"), HeaterFanState::Error(0)),
    ]
    .into_iter()
    .collect()
}

pub fn get_path<'a>(
    map: &'a HashMap<String, HeaterFanState>,
    val: &HeaterFanStateUpdateRequest,
) -> Option<&'a str> {
    match val {
        HeaterFanStateUpdateRequest::PowerOn(_) => map
            .iter()
            .find(|(_, v)| matches!(v, HeaterFanState::PowerOn(..))),
        HeaterFanStateUpdateRequest::Mode(_) => map
            .iter()
            .find(|(_, v)| matches!(v, HeaterFanState::Mode(..))),
        HeaterFanStateUpdateRequest::TargetTemperature(_) => map
            .iter()
            .find(|(_, v)| matches!(v, HeaterFanState::TargetTemperature(..))),
        HeaterFanStateUpdateRequest::FanSpeed(_) => map
            .iter()
            .find(|(_, v)| matches!(v, HeaterFanState::FanSpeed(..))),
        HeaterFanStateUpdateRequest::Oscillate(_) => map
            .iter()
            .find(|(_, v)| matches!(v, HeaterFanState::Oscillate(..))),
        HeaterFanStateUpdateRequest::Timer(_) => map
            .iter()
            .find(|(_, v)| matches!(v, HeaterFanState::Timer(..))),
        HeaterFanStateUpdateRequest::Silent(_) => map
            .iter()
            .find(|(_, v)| matches!(v, HeaterFanState::Silent(..))),
        HeaterFanStateUpdateRequest::Heater(_) => map
            .iter()
            .find(|(_, v)| matches!(v, HeaterFanState::Heater(..))),
        HeaterFanStateUpdateRequest::VentHeat(_) => map
            .iter()
            .find(|(_, v)| matches!(v, HeaterFanState::VentHeat(..))),
    }
    .map(|(k, _)| k.as_str())
}
