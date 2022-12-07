mod requests;
mod state;
mod util;

use crate::{
    config::Config,
    msg::{DeviceControl, Value},
    sensors::mqtt_heater::{
        requests::HeaterFanStateUpdateRequest,
        state::{operation_state, to_topic},
    },
};
use anyhow::{bail, Result};
use async_std::{
    sync::RwLock,
    task::{self, JoinHandle},
};
use core::time::Duration;
use futures_util::future::{self};
use log::{debug, error, info};
use rumqttc::{AsyncClient, ClientError, Event, MqttOptions, Packet, Publish, QoS};
use serde_json::Value as JsonValue;
use std::{collections::HashMap, sync::Arc};
use url::Url;
use uuid::Uuid;
use xactor::*;

use crate::msg::{ReadNow, SensorReading, SetupMetrics};

use self::{
    requests::{update_webhook_state, ThermostatState},
    state::{get_path, HeatStatus, HeaterFanState},
};

const MAX_HISTORY: usize = 1000;

#[derive(Debug)]
pub struct MqttConnection {
    options: MqttOptions,
    client: AsyncClient,
    listener_task: JoinHandle<()>,
}

#[derive(Debug)]
pub struct MqttHeaterReader {
    address: Url,
    webhook: Url,
    device_id: String,
    messages: Arc<RwLock<Vec<Publish>>>,
    collector_id: Uuid,
    name: String,
    connection: Option<MqttConnection>,
    state: Arc<RwLock<HashMap<String, HeaterFanState>>>,
}

impl MqttHeaterReader {
    pub fn new<I: Into<String>>(address: Url, webhook_url: Url, name: I, device_id: I) -> Self {
        MqttHeaterReader {
            address,
            webhook: webhook_url,
            messages: Arc::new(RwLock::new(Vec::new())),
            collector_id: Uuid::new_v4(),
            name: name.into(),
            connection: None,
            device_id: device_id.into(),
            state: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl Actor for MqttHeaterReader {
    async fn started(&mut self, ctx: &mut Context<Self>) -> anyhow::Result<()> {
        if let Some(_) = self.connection {
            panic!("Unexpected restart");
        }

        let mut options = MqttOptions::new(
            self.collector_id.as_hyphenated().to_string(),
            self.address.host_str().unwrap().to_string(),
            self.address.port().unwrap_or(1883),
        );

        options.set_keep_alive(Duration::from_secs(5));

        let (client, mut eventloop) = AsyncClient::new(options.clone(), 1000);
        info!("MQTT Connection established: {:?}", self);
        self.state = Arc::new(RwLock::new(operation_state(&self.device_id)));
        let s = self.state.read().await;

        // Subscribe to all topics
        future::join_all(s.keys().map(|t| client.subscribe(t, QoS::AtMostOnce)))
            .await
            .into_iter()
            .collect::<Result<Vec<_>, ClientError>>()
            .map_err(|e| anyhow::anyhow!(e))?;

        let state = self.state.clone();
        let messages = self.messages.clone();
        let addr = ctx.address();
        let join_handle = task::spawn(async move {
            loop {
                match eventloop.poll().await {
                    Ok(msg) => match msg {
                        Event::Incoming(e) => {
                            if let Packet::Publish(p) = e {
                                if let Ok(new_value) =
                                    serde_json::from_slice::<'_, JsonValue>(&p.payload)
                                {
                                    let topic = p.topic.clone();
                                    let mut history = messages.write().await;
                                    if history.len() >= MAX_HISTORY {
                                        history.remove(0);
                                    }
                                    history.push(p);

                                    match HeaterFanState::parse_by_key(&topic, new_value) {
                                        Ok(v) => {
                                            let mut state = state.write().await;
                                            state.insert(topic, v);
                                            let _ = addr.send(ReadNow);
                                        }
                                        Err(e) => error!("Can't find topic: {}", e),
                                    }
                                }
                            }
                        }
                        _ => {}
                    },
                    Err(e) => {
                        log::error!("Mqtt connection error: {:?}", e)
                    }
                }
            }
        });
        self.connection = Some(MqttConnection {
            options,
            client,
            listener_task: join_handle,
        });
        let mut addr = Broker::from_registry().await?;
        addr.publish(SetupMetrics::Gauge(
            self.collector_id,
            self.name.clone(),
            vec![String::from("kind"), String::from("unit")],
        ))?;

        info!("MQTT listener up");
        Ok(())
    }

    async fn stopped(&mut self, ctx: &mut Context<Self>) {
        if let Some(conn) = self.connection.take() {
            conn.listener_task.cancel().await;
        }
    }
}

impl DeviceControl {
    pub fn parsed(self) -> Result<HeaterFanStateUpdateRequest> {
        serde_json::from_slice(&self.payload).map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl Handler<ReadNow> for MqttHeaterReader {
    async fn handle(&mut self, _ctx: &mut Context<Self>, _msg: ReadNow) {
        let state = self.state.read().await;
        let data = state.values();
        let mut fan_speed = 1;
        let mut oscillate = false;
        let mut is_on = false;
        let mut current_temperature = 1_u8;
        let mut target_temperature = 1_u8;
        let mut heater = false;

        let readings: Vec<_> = data
            .filter_map(|d| match d {
                HeaterFanState::PowerOn(s) => {
                    is_on = *s;
                    Some(SensorReading {
                        id: self.collector_id,
                        reading: Value::Simple(if *s { 1.0 } else { 0.0 }),
                        labels: vec![String::from("power_on"), String::from("onoff")],
                    })
                }
                HeaterFanState::CurrentTemperature(s) => {
                    current_temperature = *s;
                    Some(SensorReading {
                        id: self.collector_id,
                        reading: Value::Simple(*s as f32),
                        labels: vec![String::from("temperature"), String::from("celsius")],
                    })
                }

                HeaterFanState::FanSpeed(s) => {
                    fan_speed = *s;
                    Some(SensorReading {
                        id: self.collector_id,
                        reading: Value::Simple(*s as f32),
                        labels: vec![String::from("fan_speed"), String::from("steps")],
                    })
                }
                HeaterFanState::Oscillate(s) => {
                    oscillate = *s;
                    Some(SensorReading {
                        id: self.collector_id,
                        reading: Value::Simple(if *s { 1.0 } else { 0.0 }),
                        labels: vec![String::from("oscillate"), String::from("onoff")],
                    })
                }
                HeaterFanState::TargetTemperature(s) => {
                    target_temperature = *s;
                    None
                }
                HeaterFanState::HeatStatus(s) => {
                    heater = s == &HeatStatus::Active;
                    None
                }
                HeaterFanState::VentHeat(s) => {
                    heater |= *s;
                    None
                }
                _ => None,
            })
            .collect();

        let mut addr = Broker::from_registry().await.unwrap();

        task::spawn(update_webhook_state(
            self.webhook.clone(),
            self.device_id.clone(),
            is_on,
            fan_speed,
            oscillate,
            current_temperature,
            target_temperature,
            heater,
        ));

        for reading in readings {
            addr.publish(reading).unwrap();
        }
    }
}

#[async_trait::async_trait]
impl Handler<DeviceControl> for MqttHeaterReader {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: DeviceControl) -> Result<()> {
        if let Some(conn) = &self.connection {
            let client = conn.client.clone();
            let parsed = msg.parsed()?;
            info!("{:?} received", parsed);
            let state = self.state.read().await;
            let topic = format!(
                "{}/set",
                get_path(&*state, &parsed).ok_or(anyhow::anyhow!("Invalid variant"))?
            );
            match parsed {
                HeaterFanStateUpdateRequest::PowerOn(new) => {
                    client
                        .publish(topic, QoS::AtLeastOnce, false, new.to_string())
                        .await
                }
                HeaterFanStateUpdateRequest::Mode(new) => {
                    client
                        .publish(topic, QoS::AtLeastOnce, false, serde_json::to_vec(&new)?)
                        .await
                }
                HeaterFanStateUpdateRequest::TargetTemperature(new) => {
                    client
                        .publish(topic, QoS::AtLeastOnce, false, new.to_string())
                        .await
                }
                HeaterFanStateUpdateRequest::FanSpeed(new) => {
                    client
                        .publish(topic, QoS::AtLeastOnce, false, new.to_string())
                        .await
                }
                HeaterFanStateUpdateRequest::Oscillate(new) => {
                    client
                        .publish(topic, QoS::AtLeastOnce, false, new.to_string())
                        .await
                }
                HeaterFanStateUpdateRequest::Timer(new) => {
                    client
                        .publish(topic, QoS::AtLeastOnce, false, new.to_string())
                        .await
                }
                HeaterFanStateUpdateRequest::Silent(new) => {
                    client
                        .publish(topic, QoS::AtLeastOnce, false, new.to_string())
                        .await
                }
                HeaterFanStateUpdateRequest::Heater(new) => {
                    let power_topic = to_topic(&self.device_id, "power_on");
                    if new == ThermostatState::Off {
                        client
                            .publish(power_topic, QoS::AtLeastOnce, false, false.to_string())
                            .await
                    } else {
                        if let Some(power_on) = state.get(&power_topic) {
                            if power_on != &HeaterFanState::PowerOn(true) {
                                client
                                    .publish(
                                        format!("{}/set", power_topic),
                                        QoS::AtLeastOnce,
                                        false,
                                        true.to_string(),
                                    )
                                    .await?;
                            }
                        }
                        client
                            .publish(
                                topic,
                                QoS::AtLeastOnce,
                                false,
                                (new == ThermostatState::Heating).to_string(),
                            )
                            .await
                    }
                }
                _ => Ok(()),
            }
            .map_err(|e| e.into())
        } else {
            bail!("MQTT not connected");
        }
    }
}

pub async fn setup(config: &Config) -> Result<Addr<MqttHeaterReader>> {
    MqttHeaterReader::new(
        config.mqtt_address()?,
        config.webhook_url()?,
        &config.metrics_name,
        &config.heaterfan_mac()?,
    )
    .start()
    .await
}
