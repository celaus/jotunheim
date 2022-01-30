use crate::{config::Config, msg::Value};
use async_std::task;
use core::time::Duration;
use log::{debug, error, info};
use serde_json;
use std::process::{Command, Stdio};
use uuid::Uuid;
use xactor::*;

use crate::msg::{ReadNow, SensorReading, SetupMetrics};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
struct ExternalReading {
    value: f64,
    kind: String,
    unit: String,
}

pub struct ExternalSensorReader {
    path: String,
    args: Vec<String>,
    resolution: Duration,
    collector_id: Uuid,
    name: String,
}

impl ExternalSensorReader {
    pub fn new<I: Into<String>>(path: I, name: I, args: Vec<String>, resolution: Duration) -> Self {
        let collector_id = Uuid::new_v4();
        ExternalSensorReader {
            path: path.into(),
            args,
            collector_id,
            resolution,
            name: name.into()
        }
    }
}

#[async_trait::async_trait]
impl Actor for ExternalSensorReader {
    async fn started(&mut self, ctx: &mut Context<Self>) -> anyhow::Result<()> {
        let mut addr = Broker::from_registry().await?;
        addr.publish(SetupMetrics::Gauge(
            self.collector_id,
            self.name.clone(),
            vec![String::from("kind"), String::from("unit")],
        ))?;

        ctx.send_interval(ReadNow, self.resolution);
        info!("External reader for path '{}' set up", self.path);
        debug!(
            "Expecting JSON cmd line output like: {}",
            serde_json::to_string(&ExternalReading::default()).unwrap()
        );
        Ok(())
    }
}

#[async_trait::async_trait]
impl Handler<ReadNow> for ExternalSensorReader {
    async fn handle(&mut self, _ctx: &mut Context<Self>, _msg: ReadNow) {
        let exe = self.path.clone();
        let default_args = self.args.clone();
        debug!("Starting execution with {}", exe);
        let output: String = task::spawn_blocking(move || -> anyhow::Result<String> {
            let child = Command::new(&*exe)
                .args(default_args)
                .env_clear()
                .stdout(Stdio::piped())
                .spawn()
                .expect("failed to execute child");

            let output = child.wait_with_output().expect("failed to wait on child");
            if output.status.success() {
                std::str::from_utf8(&output.stdout)
                    .map(|s| s.to_owned())
                    .map_err(|e| e.into())
            } else {
                error!(
                    "Command '{}' returned a non-zero exit code: {:?}",
                    exe,
                    output.status.code()
                );
                Err(std::io::Error::from(std::io::ErrorKind::NotFound).into())
            }
        })
        .await
        .expect("Command didn't complete successfully.");

        let ext_values: Vec<ExternalReading> =
            serde_json::from_str(&output).expect("No valid JSON was returned");

        let readings = ext_values.into_iter().map(|v| SensorReading {
            reading: Value::Simple(v.value as f32),
            id: self.collector_id,
            labels: vec![v.kind, v.unit],
        });

        let mut addr = Broker::from_registry().await.unwrap();
        for reading in readings {
            addr.publish(reading).unwrap();
        }
    }
}

pub async fn setup(config: &Config) -> Result<Vec<Addr<ExternalSensorReader>>> {
    let mut external_actors = vec![];
    let externals = config.parsed_externals().await;

    if !externals.is_empty() {
        info!(
            "External Sensor module active, {} paths found",
            externals.len()
        );
        for actor in externals
            .into_iter()
            .map(|p| ExternalSensorReader::new(&p, &config.metrics_name,vec![], Duration::from_secs(1)))
        {
            let a = actor.start().await?;
            external_actors.push(a);
        }
        Ok(external_actors)
    } else {
        Ok(vec![])
    }
}
