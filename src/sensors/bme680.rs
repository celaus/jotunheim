use crate::{msg::Value, AccessoryType};
use bme680::*;
use core::time::Duration;
use hal::I2cdev;
use linux_embedded_hal as hal;
use log::{debug, info};
use std::path::Path;
use uuid::Uuid;
use xactor::*;

use anyhow::Result;

use crate::msg::{ReadNow, SensorReading, SetupMetrics};

struct AsyncDelay {}

impl embedded_hal::blocking::delay::DelayMs<u8> for AsyncDelay {
    fn delay_ms(&mut self, ms: u8) {
        debug!("Delay called: {}ms", ms);
        async_std::task::block_on(async_std::task::sleep(Duration::from_millis(ms.into())));
    }
}

pub fn is_available(path: &str) -> bool {
    Path::new(path).exists()
}

pub(crate) struct Bme680SensorReader {
    dev: Bme680<I2cdev, AsyncDelay>,
    collector_id: Uuid,
    resolution: Duration,
    name: String,
}

// "/dev/i2c-1"
impl Bme680SensorReader {
    pub fn new<I: Into<String>>(path: &str, name: I, resolution: Duration) -> Result<Self> {
        let i2c = I2cdev::new(path)?;
        let mut dev = Bme680::init(i2c, &mut AsyncDelay {}, I2CAddress::Primary).unwrap();
        let collector_id = Uuid::new_v4();
        Ok(Bme680SensorReader {
            dev,
            collector_id,
            resolution,
            name: name.into(),
        })
    }
}

#[async_trait::async_trait]
impl Actor for Bme680SensorReader {
    async fn started(&mut self, ctx: &mut Context<Self>) -> anyhow::Result<()> {
        let mut addr = Broker::from_registry().await?;
        addr.publish(SetupMetrics::Gauge(
            self.collector_id,
            self.name.clone(),
            vec![String::from("kind"), String::from("unit")],
        ))?;

        ctx.send_interval(ReadNow, self.resolution);
        info!("BME680 reader set up");
        Ok(())
    }
}

#[async_trait::async_trait]
impl Handler<ReadNow> for Bme680SensorReader {
    async fn handle(&mut self, _ctx: &mut Context<Self>, _msg: ReadNow) {
        info!("READING");
        let settings = SettingsBuilder::new()
            .with_humidity_oversampling(OversamplingSetting::OS2x)
            .with_pressure_oversampling(OversamplingSetting::OS4x)
            .with_temperature_oversampling(OversamplingSetting::OS8x)
            .with_temperature_filter(IIRFilterSize::Size3)
            .with_gas_measurement(Duration::from_millis(1500), 320, 25)
            .with_temperature_offset(-2.2)
            .with_run_gas(true)
            .build();

        let profile_dur = self.dev.get_profile_dur(&settings.0).unwrap();

        info!("Profile duration {:?}", profile_dur);
        info!("Setting sensor settings");
        self.dev
            .set_sensor_settings(&mut AsyncDelay {}, settings)
            .unwrap();
        info!("Setting forced power modes");
        self.dev
            .set_sensor_mode(&mut AsyncDelay {}, PowerMode::ForcedMode)
            .unwrap();

        let sensor_settings = self.dev.get_sensor_settings(settings.1);
        info!("Sensor settings: {:?}", sensor_settings);

        let power_mode = self.dev.get_sensor_mode();
        info!("Sensor power mode: {:?}", power_mode);
        info!("Setting forced power modes");
        self.dev
            .set_sensor_mode(&mut AsyncDelay {}, PowerMode::ForcedMode)
            .unwrap();
        info!("Retrieving sensor data");
        let (data, _state) = self.dev.get_sensor_data(&mut AsyncDelay {}).unwrap();
        info!("Sensor Data {:?}", data);
        info!("Temperature {}°C", data.temperature_celsius());
        info!("Pressure {}hPa", data.pressure_hpa());
        info!("Humidity {}%", data.humidity_percent());
        info!("Gas Resistence {}Ω", data.gas_resistance_ohm());
        let readings = vec![
            SensorReading {
                id: self.collector_id,
                reading: Value::Simple(data.temperature_celsius()),
                labels: vec![String::from("temperature"), String::from("celsius")],
                accessory_type: AccessoryType::Temperature,
            },
            SensorReading {
                id: self.collector_id,
                reading: Value::Simple(data.pressure_hpa()),
                labels: vec![String::from("pressure"), String::from("hpa")],
                accessory_type: AccessoryType::Pressure,
            },
            SensorReading {
                id: self.collector_id,
                reading: Value::Simple(data.humidity_percent()),
                labels: vec![String::from("humidity"), String::from("percent")],
                accessory_type: AccessoryType::Humidity,
            },
            SensorReading {
                id: self.collector_id,
                reading: Value::Simple(data.gas_resistance_ohm() as f32),
                labels: vec![String::from("gas_resistance"), String::from("ohm")],
                accessory_type: AccessoryType::GasResistance,
            },
        ];

        let mut addr = Broker::from_registry().await.unwrap();
        for reading in readings {
            addr.publish(reading).unwrap();
        }
    }
}
