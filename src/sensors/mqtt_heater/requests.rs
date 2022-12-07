use super::util::bool_from_int;
use super::{state::HeaterFanMode, util::int_from_bool};
use futures_util::future::join_all;
use log::{debug, error, trace};
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use surf::RequestBuilder;
use url::Url;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HomeBridgeWebHookRequest<T: Serialize> {
    accessory_id: String,
    #[serde(flatten)]
    state: T,
}

#[derive(Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum HomeBridgeFanv2 {
    PowerOnOff {
        #[serde(serialize_with = "int_from_bool")]
        state: bool,
    },

    RotationSpeed {
        speed: u8,
    }, //Rotation Speed (%): http://yourHomebridgeServerIp:webhook_port/?accessoryId=theAccessoryIdToUpdate&speed=%s (%s is replaced by fan's rotation speed)

    #[serde(rename_all = "camelCase")]
    Oscillate {
        #[serde(serialize_with = "int_from_bool")]
        swing_mode: bool,
    }, //Swing Mode (DISABLED=0 / ENABLED=1): http://yourHomebridgeServerIp:webhook_port/?accessoryId=theAccessoryIdToUpdate&swingMode=0 (or 1)
       //Rotation Direction (CLOCKWISE=0 / COUNTER_CLOCKWISE=1): http://yourHomebridgeServerIp:webhook_port/?accessoryId=theAccessoryIdToUpdate&rotationDirection=0 (or 1)
}

#[derive(Serialize_repr, Deserialize_repr, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ThermostatState {
    Off = 0,
    Heating = 1,
    Cooling = 2,
    Auto = 3,
}

#[derive(Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum HomeBridgeThermostat {
    #[serde(rename_all = "camelCase")]
    CurrentState { current_state: ThermostatState },
    #[serde(rename_all = "camelCase")]
    TargetState { target_state: ThermostatState },
    #[serde(rename_all = "camelCase")]
    CurrentTemperature { current_temperature: f64 },
    #[serde(rename_all = "camelCase")]
    TargetTemperature { target_temperature: f64 },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum HeaterFanStateUpdateRequest {
    PowerOn(bool),
    Mode(HeaterFanMode),
    TargetTemperature(u8),
    FanSpeed(u8), // 1-10
    #[serde(deserialize_with = "bool_from_int")]
    Oscillate(bool),
    Timer(u16),
    Silent(bool),
    Heater(ThermostatState),
    VentHeat(bool),
}

pub async fn update_webhook_state(
    webhook_url: Url,
    device_id: String,
    is_on: bool,
    fan_speed: u8,
    oscillate: bool,
    current_temperature: u8,
    target_temperature: u8,
    heater_on: bool,
) -> anyhow::Result<()> {
    let client = surf::client();
    let accessory_id_h = format!("t-{}", device_id);
    let current_temp_payload = HomeBridgeWebHookRequest {
        accessory_id: accessory_id_h.clone(),
        state: HomeBridgeThermostat::CurrentTemperature {
            current_temperature: current_temperature as f64,
        },
    };
    let target_temp_payload = HomeBridgeWebHookRequest {
        accessory_id: accessory_id_h.clone(),
        state: HomeBridgeThermostat::TargetTemperature {
            target_temperature: target_temperature as f64,
        },
    };
    let heater_payload = HomeBridgeWebHookRequest {
        accessory_id: accessory_id_h.clone(),
        state: HomeBridgeThermostat::CurrentState {
            current_state: find_current_state(is_on, heater_on),
        },
    };

    let speed_payload = HomeBridgeWebHookRequest {
        accessory_id: device_id.clone(),
        state: HomeBridgeFanv2::RotationSpeed { speed: fan_speed },
    };
    let power_payload = HomeBridgeWebHookRequest {
        accessory_id: device_id.clone(),
        state: HomeBridgeFanv2::PowerOnOff { state: is_on },
    };

    let oscillate_payload = HomeBridgeWebHookRequest {
        accessory_id: device_id,
        state: HomeBridgeFanv2::Oscillate {
            swing_mode: oscillate,
        },
    };

    let rqs = vec![
        create_req(&webhook_url, &current_temp_payload)?,
        create_req(&webhook_url, &target_temp_payload)?,
        create_req(&webhook_url, &heater_payload)?,
        create_req(&webhook_url, &speed_payload)?,
        create_req(&webhook_url, &power_payload)?,
        create_req(&webhook_url, &oscillate_payload)?,
    ]
    .into_iter()
    .map(|r| client.send(r));
    for res in join_all(rqs).await {
        match res {
            Ok(content) => {
                trace!("Webhook call response: {:?}", content);
            }
            Err(err) => {
                error!("Webhook call returned an error: {:?}", err);
                return Err(err.into_inner().into());
            }
        }
    }
    Ok(())
}

fn create_req<T: Serialize>(webhook_url: &Url, payload: &T) -> anyhow::Result<RequestBuilder> {
    let rb = surf::get(webhook_url)
        .query(payload)
        .map_err(|e| e.into_inner())?;
    debug!("{:?}", rb.build());
    surf::get(webhook_url)
        .query(payload)
        .map_err(|e| e.into_inner())
}

fn find_current_state(is_on: bool, heater_on: bool) -> ThermostatState {
    if !is_on {
        ThermostatState::Off
    } else {
        if heater_on {
            ThermostatState::Heating
        } else {
            ThermostatState::Cooling
        }
    }
}
