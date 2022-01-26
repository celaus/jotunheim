use crate::{msg::Value, utils::e_, AccessoryType};
use async_std::task;
use core::time::Duration;
use log::{debug, error, info};
use serde_json;
use std::process::{Command, Stdio};
use uuid::Uuid;
use xactor::*;

use crate::msg::{ReadNow, SensorReading, SetupMetrics};

use serde::{Deserialize, Serialize};
const AUTH_URL: &str = "https://api.netatmo.com/oauth2/token";
const DATA_URL: &str = "https://api.netatmo.com/getstationsdata";

#[message]
#[derive(Clone, Debug)]
enum IntervalMessage {
    Refresh,
    Read,
}

#[derive(Serialize, Deserialize, Default)]
struct AuthResponse {
    pub access_token: String,
    pub expires_in: usize,
    pub refresh_token: String,
}

#[derive(Serialize, Deserialize, Default)]
struct AuthRefreshRequest {
    pub grant_type: String,    //=refresh_token
    pub refresh_token: String, //=[YOUR_REFRESH_TOKEN]
    pub client_id: String,     //=[YOUR_APP_ID]
    pub client_secret: String, //=[YOUR_CLIENT_SECRET]
}

impl AuthRefreshRequest {
    pub fn create_from(previous: &AuthResponse, client: &NetatmoSingleAuth) -> Self {
        AuthRefreshRequest {
            grant_type: "refresh_token".into(),
            refresh_token: previous.refresh_token.clone(), //=[YOUR_REFRESH_TOKEN]
            client_id: client.client_id.clone(),
            client_secret: client.client_secret.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct NetatmoResponseWithBody {
    pub body: serde_json::Value,
    pub status: String,
    pub time_exec: f32,
    pub time_server: usize,
}

async fn handle_auth<A: Serialize>(a: &A) -> Result<AuthResponse> {
    let client = reqwest::Client::new();
    client
        .post(AUTH_URL)
        .body(serde_urlencoded::to_string(a)?)
        .header(
            "Content-Type",
            "application/x-www-form-urlencoded;charset=UTF-8",
        )
        .send()
        .await?
        .json::<AuthResponse>()
        .await
        .map_err(e_)
}

#[derive(Debug, Serialize, Default)]
pub struct NetatmoSingleAuth {
    pub grant_type: String,    //=refresh_token
    pub client_id: String,     //=[YOUR_APP_ID]
    pub client_secret: String, //=[YOUR_CLIENT_SECRET]
    pub username: String,      //=[USER_MAIL]
    pub password: String,      //=[USER_PASSWORD]
    pub scope: String,         //=[SCOPES_SPACE_SEPARATED]
}

impl NetatmoSingleAuth {
    pub fn with_scope_read_station(
        client_id: String,
        client_secret: String,
        username: String,
        password: String,
    ) -> Self {
        NetatmoSingleAuth {
            grant_type: "password".into(),
            client_id,
            client_secret,
            username,
            password,
            scope: "read_station".into(),
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
struct ExternalReading {
    value: f64,
    kind: String,
    unit: String,
    accessory_type: AccessoryType,
}

pub(crate) struct NetatmoSensorReader {
    device_id: String,
    auth: NetatmoSingleAuth,
    auth_token: Option<AuthResponse>,
    resolution: Duration,
    collector_id: Uuid,
}

impl NetatmoSensorReader {
    pub fn new<I: Into<String>>(
        device_id: I,
        auth_data: NetatmoSingleAuth,
        resolution: Duration,
    ) -> Self {
        let collector_id = Uuid::new_v4();
        NetatmoSensorReader {
            device_id: device_id.into(),
            auth: auth_data,
            auth_token: None,
            collector_id,
            resolution,
        }
    }
}

#[async_trait::async_trait]
impl Actor for NetatmoSensorReader {
    async fn started(&mut self, ctx: &mut Context<Self>) -> anyhow::Result<()> {
        let token = handle_auth(&self.auth).await?;
        let when_refresh = Duration::from_secs((token.expires_in - 60) as u64);
        ctx.send_later(IntervalMessage::Refresh, when_refresh);

        self.auth_token = Some(token);

        let mut addr = Broker::from_registry().await?;

        addr.publish(SetupMetrics::Gauge(
            self.collector_id,
            "netatmo".into(),
            vec![String::from("kind"), String::from("unit")],
        ))?;

        ctx.send_interval(IntervalMessage::Read, self.resolution);
        info!("Netatmo reader set up");
        debug!(
            "Expecting JSON cmd line output like: {}",
            serde_json::to_string(&ExternalReading::default()).unwrap()
        );
        Ok(())
    }
}

#[async_trait::async_trait]
impl Handler<IntervalMessage> for NetatmoSensorReader {
    async fn handle(&mut self, ctx: &mut Context<Self>, msg: IntervalMessage) {
        match msg {
            IntervalMessage::Read => {
                if let Some(auth) = &self.auth_token {
                    let auth_header = format!("BEARER {}", auth.access_token);
                    let client = reqwest::Client::new();
                    let url = format!("{}?station_id={}", DATA_URL, self.device_id);
                    match client
                        .get(&url)
                        .header("Authorization", auth_header)
                        .send()
                        .await
                    {
                        Ok(resp) => {
                            let response: NetatmoResponseWithBody =
                                resp.json().await.expect("weird response");

                            let data = &response.body["devices"][0]["dashboard_data"];

                            let readings = vec![
                                SensorReading {
                                    id: self.collector_id,
                                    reading: Value::Simple(
                                        data["Temperature"].as_f64().unwrap() as f32
                                    ),
                                    labels: vec![
                                        String::from("temperature"),
                                        String::from("celsius"),
                                    ],
                                    accessory_type: AccessoryType::Temperature,
                                },
                                SensorReading {
                                    id: self.collector_id,
                                    reading: Value::Simple(
                                        data["Pressure"].as_f64().unwrap() as f32
                                    ), // AbsolutePressure ?
                                    labels: vec![String::from("pressure"), String::from("hpa")],
                                    accessory_type: AccessoryType::Pressure,
                                },
                                SensorReading {
                                    id: self.collector_id,
                                    reading: Value::Simple(
                                        data["Humidity"].as_f64().unwrap() as f32
                                    ),
                                    labels: vec![String::from("humidity"), String::from("percent")],
                                    accessory_type: AccessoryType::Humidity,
                                },
                                SensorReading {
                                    id: self.collector_id,
                                    reading: Value::Simple(data["CO2"].as_f64().unwrap() as f32),
                                    labels: vec![
                                        String::from("gas_resistance"),
                                        String::from("ohm"),
                                    ],
                                    accessory_type: AccessoryType::GasResistance,
                                },
                            ];
                            let mut addr = Broker::from_registry().await.unwrap();
                            for reading in readings {
                                addr.publish(reading).unwrap();
                            }
                        }
                        Err(e) => {
                            error!("API responded with an error: {:?}", e);
                        }
                    }
                }
            }
            IntervalMessage::Refresh => {
                match handle_auth(&AuthRefreshRequest::create_from(
                    &self.auth_token.as_ref().unwrap(),
                    &self.auth,
                ))
                .await
                {
                    Ok(token) => {
                        let when_refresh = Duration::from_secs((token.expires_in - 60) as u64);
                        ctx.send_later(IntervalMessage::Refresh, when_refresh);
                    }
                    Err(e) => {
                        error!("Couldn't refresh auth token: {:?}", e);
                    }
                }
            }
        }
    }
}
