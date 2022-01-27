use crate::{config::Config, extract_from, msg::Value, utils::avg, AccessoryType};

use anyhow::anyhow;
use async_std::task;
use core::time::Duration;
use futures_util::{join, FutureExt};
use log::{debug, error, info};
use serde_json;
use uuid::Uuid;
use xactor::*;

use crate::msg::{SensorReading, SetupMetrics};
use serde::{Deserialize, Serialize};

const AUTH_URL: &str = "https://api.netatmo.com/oauth2/token";
const PRIVATE_URL: &str = "https://api.netatmo.com/api/getstationsdata";
const PUBLIC_URL: &str = "https://api.netatmo.com/api/getpublicdata";

#[derive(Serialize, Deserialize)]
struct PrivateDataQuery {
    device_id: String,
    get_favorites: bool,
}

#[derive(Serialize, Deserialize)]
struct GetPublicDataQuery {
    lat_ne: f64,
    lon_ne: f64,
    lat_sw: f64,
    lon_sw: f64,
    filter: bool,
}

enum Reading {
    Wind(f64),
    Rain(f64),
}

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
    pub time_exec: Option<f32>,
    pub time_server: Option<usize>,
}

async fn handle_auth<A: Serialize>(a: &A) -> Result<AuthResponse> {
    let payload = serde_urlencoded::to_string(a)?;
    surf::post(AUTH_URL)
        .body(payload)
        .header(
            "Content-Type",
            "application/x-www-form-urlencoded;charset=UTF-8",
        )
        .recv_json::<AuthResponse>()
        .await
        .map_err(|e| {
            error!("Authentication Error {:?}", e);
            e.into_inner()
        })
}

#[derive(Debug, Serialize, Default, PartialEq, Eq)]
pub struct NetatmoSingleAuth {
    pub grant_type: String,    //=password
    pub client_id: String,     //=[YOUR_APP_ID]
    pub client_secret: String, //=[YOUR_CLIENT_SECRET]
    pub username: String,      //=[USER_MAIL]
    pub password: String,      //=[USER_PASSWORD]
    pub scope: String,         //=[SCOPES_SPACE_SEPARATED]
}

impl NetatmoSingleAuth {
    pub fn with_scope_read_station<S: Into<String>>(
        client_id: S,
        client_secret: S,
        username: S,
        password: S,
    ) -> Self {
        NetatmoSingleAuth {
            grant_type: "password".into(),
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            username: username.into(),
            password: password.into(),
            scope: "read_station".into(),
        }
    }
}

pub struct NetatmoSensorReader {
    device_id: String,
    auth: NetatmoSingleAuth,
    auth_token: Option<AuthResponse>,
    resolution: Duration,
    collector_id: Uuid,
    location: ((f64, f64), (f64, f64)),
}

impl NetatmoSensorReader {
    pub fn new<I: Into<String>>(
        device_id: I,
        auth_data: NetatmoSingleAuth,
        resolution: Duration,
        location: ((f64, f64), (f64, f64)),
    ) -> Self {
        let collector_id = Uuid::new_v4();
        NetatmoSensorReader {
            device_id: device_id.into(),
            auth: auth_data,
            auth_token: None,
            collector_id,
            resolution,
            location,
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
        Ok(())
    }
}

#[async_trait::async_trait]
impl Handler<IntervalMessage> for NetatmoSensorReader {
    async fn handle(&mut self, ctx: &mut Context<Self>, msg: IntervalMessage) {
        match msg {
            IntervalMessage::Read => {
                if let Some(auth) = &self.auth_token {
                    let auth_header = format!("Bearer {}", auth.access_token);
                    let addr_ = Broker::from_registry().await.unwrap();

                    let ((lon_sw, lat_sw), (lon_ne, lat_ne)) = self.location;
                    debug!(
                        "Querying Rectangle: NE({}, {}) - SW({}, {})",
                        lat_ne, lon_ne, lat_sw, lon_sw
                    );
                    let public_data_params = GetPublicDataQuery {
                        lat_ne,
                        lon_ne,
                        lat_sw,
                        lon_sw,
                        filter: true,
                    };

                    let collector_id = self.collector_id.clone();
                    let mut addr = addr_.clone();
                    let public_data = task::spawn(
                        surf::get(&PUBLIC_URL)
                            .query(&public_data_params)
                            .unwrap()
                            .header("Authorization", auth_header)
                            .header("accept", "application/json")
                            .recv_json::<NetatmoResponseWithBody>()
                            .then(move |response| async move {
                                match response {
                                    Ok(response) => {
                                        let readings: Vec<_> = response
                                            .body
                                            .as_array()
                                            .unwrap()
                                            .iter()
                                            .filter_map(|v| {
                                                if v["place"]["city"] == "Amsterdam" {
                                                    v["measures"]
                                                        .as_object()
                                                        .map(|o| o.values().collect::<Vec<_>>())
                                                } else {
                                                    None
                                                }
                                            })
                                            .flatten()
                                            .filter_map(|v| {
                                                v.as_object().and_then(|m| {
                                                    m.get("rain_live")
                                                        .and_then(|r| r.as_f64())
                                                        .and_then(|r| Some(Reading::Rain(r)))
                                                        .or(m
                                                            .get("wind_strength")
                                                            .and_then(|r| r.as_f64())
                                                            .and_then(|r| Some(Reading::Wind(r))))
                                                })
                                            })
                                            .collect();

                                        let wind = extract_from!(readings.iter(), Reading::Wind);
                                        let rain = extract_from!(readings.iter(), Reading::Rain);

                                        addr.publish(SensorReading {
                                            id: collector_id,
                                            reading: Value::Simple(avg(&wind) as f32),
                                            labels: vec![String::from("wind"), String::from("kph")],
                                            accessory_type: AccessoryType::Wind,
                                        })
                                        .unwrap();

                                        addr.publish(SensorReading {
                                            id: collector_id,
                                            reading: Value::Simple(avg(&rain) as f32),
                                            labels: vec![String::from("rain"), String::from("mm")],
                                            accessory_type: AccessoryType::Rain,
                                        })
                                        .unwrap();
                                    }
                                    Err(e) => error!("Public API responded with an error: {:?}", e),
                                }
                            }),
                    );
                    let params = PrivateDataQuery {
                        device_id: self.device_id.clone(),
                        get_favorites: false,
                    };
                    let collector_id = self.collector_id.clone();
                    let auth_header = format!("Bearer {}", auth.access_token);
                    let mut addr = addr_.clone();
                    let local_data = task::spawn(
                        surf::get(PRIVATE_URL)
                            .query(&params)
                            .unwrap()
                            .header("Authorization", auth_header)
                            .header("accept", "application/json")
                            .recv_json::<NetatmoResponseWithBody>()
                            .then(move |response| async move {
                                match response {
                                    Ok(response) => {
                                        let data = &response.body["devices"][0]["dashboard_data"];
                                        let readings = vec![
                                            SensorReading {
                                                id: collector_id,
                                                reading: Value::Simple(
                                                    data["Temperature"].as_f64().unwrap() as f32,
                                                ),
                                                labels: vec![
                                                    String::from("temperature"),
                                                    String::from("celsius"),
                                                ],
                                                accessory_type: AccessoryType::Temperature,
                                            },
                                            SensorReading {
                                                id: collector_id,
                                                reading: Value::Simple(
                                                    data["Pressure"].as_f64().unwrap() as f32,
                                                ), // AbsolutePressure ?
                                                labels: vec![
                                                    String::from("pressure"),
                                                    String::from("hpa"),
                                                ],
                                                accessory_type: AccessoryType::Pressure,
                                            },
                                            SensorReading {
                                                id: collector_id,
                                                reading: Value::Simple(
                                                    data["Humidity"].as_f64().unwrap() as f32,
                                                ),
                                                labels: vec![
                                                    String::from("humidity"),
                                                    String::from("percent"),
                                                ],
                                                accessory_type: AccessoryType::Humidity,
                                            },
                                            SensorReading {
                                                id: collector_id,
                                                reading: Value::Simple(
                                                    data["CO2"].as_f64().unwrap() as f32,
                                                ),
                                                labels: vec![
                                                    String::from("co2"),
                                                    String::from("ppm"),
                                                ],
                                                accessory_type: AccessoryType::Co2,
                                            },
                                        ];
                                        for reading in readings {
                                            addr.publish(reading).unwrap();
                                        }
                                    }
                                    Err(e) => error!("API responded with an error: {:?}", e),
                                }
                            }),
                    );

                    join!(local_data, public_data);
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

pub async fn setup(config: &Config) -> Result<Addr<NetatmoSensorReader>> {
    //"id|user|password|clientid|secret"
    let all_creds = config.parsed_credentials().await;
    let raw_creds = all_creds
        .get("netatmo")
        .ok_or(anyhow!("No netatmo credentials found"))?;
    let (device_id, parsed_credentials) = parse(&raw_creds)?;
    NetatmoSensorReader::new(
        device_id,
        parsed_credentials,
        config.resolution(),
        config.location_rect()?,
    )
    .start()
    .await
}

fn parse(creds: &str) -> Result<(String, NetatmoSingleAuth)> {
    let mut cred_iterator = creds.split("|").map(|s| s.trim());
    let device_id = cred_iterator
        .next()
        .ok_or(anyhow!("No netatmo device_id found"))?;
    let user = cred_iterator
        .next()
        .ok_or(anyhow!("No netatmo user found"))?;
    let password = cred_iterator
        .next()
        .ok_or(anyhow!("No netatmo password found"))?;
    let client_id = cred_iterator
        .next()
        .ok_or(anyhow!("No netatmo clientid found"))?;
    let secret = cred_iterator
        .next()
        .ok_or(anyhow!("No netatmo secret found"))?;
    let auth = NetatmoSingleAuth::with_scope_read_station(client_id, secret, user, password);
    Ok((device_id.to_string(), auth))
}

#[cfg(test)]
mod tests {
    #![allow(non_snake_case)]
    use super::*;

    #[async_std::test]
    async fn test_parse_credentials_separates_well() {
        let credential_str = "id|user|password|clientid|secret";
        let expected = (
            "id".to_string(),
            NetatmoSingleAuth::with_scope_read_station("clientid", "secret", "user", "password"),
        );
        let actual = parse(credential_str).unwrap();
        assert_eq!(actual.0, expected.0);
        assert_eq!(actual.1, expected.1);
    }
}
