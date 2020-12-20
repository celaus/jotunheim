use bme680::*;
use core::result;
use core::time::Duration;
use embedded_hal::blocking::i2c;
use futures_util::future::UnsafeFutureObj;
use hal::I2cdev;
use linux_embedded_hal as hal;
use log::{info, debug};
use uuid::Uuid;
use xactor::*;
use crate::msg::Value;

use anyhow::Result;

use crate::{
    msg::{ReadNow, SensorReading, SetupMetrics},
    CollectorAddr,
};

struct AsyncDelay {}

impl embedded_hal::blocking::delay::DelayMs<u8> for AsyncDelay {
    fn delay_ms(&mut self, ms: u8) {
        debug!("Delay called: {}ms", ms);
        async_std::task::block_on(async_std::task::sleep(Duration::from_millis(ms.into())));
    }
}

pub(crate) struct Bme680SensorReader {
    dev: Bme680<I2cdev, AsyncDelay>,
    collector_id: Uuid,
    resolution: Duration
}

// "/dev/i2c-1"
impl Bme680SensorReader {
    pub fn new(path: &str, collector_id: Uuid, resolution: Duration) -> Result<Self> {
        let i2c = I2cdev::new(path)?;
        let mut dev = Bme680::init(i2c, AsyncDelay {}, I2CAddress::Primary).unwrap();
        Ok(Bme680SensorReader { dev, collector_id, resolution })
    }
}

#[async_trait::async_trait]
impl Actor for Bme680SensorReader {
    async fn started(&mut self, ctx: &mut Context<Self>) -> anyhow::Result<()> {
        ctx.send_interval_with(|| ReadNow{}, self.resolution);
        Ok(())
    }
}

#[async_trait::async_trait]
impl Handler<ReadNow> for Bme680SensorReader {
    async fn handle(&mut self, _ctx: &mut Context<Self>, _msg: ReadNow) {
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
        self.dev.set_sensor_settings(settings).unwrap();
        info!("Setting forced power modes");
        self.dev.set_sensor_mode(PowerMode::ForcedMode).unwrap();

        let sensor_settings = self.dev.get_sensor_settings(settings.1);
        info!("Sensor settings: {:?}", sensor_settings);

        let power_mode = self.dev.get_sensor_mode();
        info!("Sensor power mode: {:?}", power_mode);
        info!("Setting forced power modes");
        self.dev.set_sensor_mode(PowerMode::ForcedMode).unwrap();
        info!("Retrieving sensor data");
        let (data, _state) = self.dev.get_sensor_data().unwrap();
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
            },
            SensorReading {
                id: self.collector_id,
                reading: Value::Simple(data.pressure_hpa()),
                labels: vec![String::from("pressure"), String::from("hpa")],
            },
            SensorReading {
                id: self.collector_id,
                reading: Value::Simple(data.humidity_percent()),
                labels: vec![String::from("humidity"), String::from("percent")],
            },
            SensorReading {
                id: self.collector_id,
                reading: Value::Simple(data.gas_resistance_ohm()as f32),
                labels: vec![String::from("gas_resistance"), String::from("ohm")],
            },
        ];

        let mut addr = Broker::from_registry().await.unwrap();
        for reading in readings {
            addr.publish(reading).unwrap();
        }

    }
}
