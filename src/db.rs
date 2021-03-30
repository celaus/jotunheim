use std::collections::HashMap;

use crate::msg::{EncodeData, SensorReading, SetupMetrics};
use log::{error, info};
use prometheus::{
    CounterVec, Encoder, GaugeVec, IntCounter, IntCounterVec, IntGauge, IntGaugeVec, Opts,
    Registry, TextEncoder,
};
use uuid::Uuid;
use xactor::*;

enum DataCollector {
    Gauge(GaugeVec),
    Counter(CounterVec),
}

impl DataCollector {
    pub fn set(&self, label_vals: &[&str], value: f64) {
        match self {
            DataCollector::Gauge(c) => {
                c.with_label_values(label_vals).set(value);
            }
            _ => {}
        }
    }
    pub fn inc(&self, label_vals: &[&str]) {
        match self {
            DataCollector::Gauge(c) => {
                c.with_label_values(label_vals).inc();
            }
            DataCollector::Counter(c) => {
                c.with_label_values(label_vals).inc();
            }
        }
    }
    pub fn dec(&self, label_vals: &[&str]) {
        match self {
            DataCollector::Gauge(c) => {
                c.with_label_values(label_vals).dec();
            }
            _ => {}
        }
    }
}

pub(crate) struct PrometheusCollector {
    registry: Registry,
    metrics: HashMap<Uuid, DataCollector>,
}

impl PrometheusCollector {
    pub fn new() -> Result<Self> {
        let registry = Registry::new();
        Ok(PrometheusCollector {
            registry,
            metrics: HashMap::new(),
        })
    }
}

#[async_trait::async_trait]
impl Actor for PrometheusCollector {
    async fn started(&mut self, ctx: &mut Context<Self>) -> Result<()> {
        ctx.subscribe::<SetupMetrics>().await?;
        ctx.subscribe::<SensorReading>().await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl Handler<SetupMetrics> for PrometheusCollector {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: SetupMetrics) {
        info!("Setting up: {:?}", msg);
        match msg {
            SetupMetrics::Gauge(id, name, labels) => {
                let options = Opts::new(name, "help");
                let label_names: Vec<&str> = labels.iter().map(|s| &**s).collect();
                let gauge = GaugeVec::new(options, &label_names).expect("Couldn't create Gauge");
                self.registry
                    .register(Box::new(gauge.clone()))
                    .expect("Couldn't register metric to prometheus");
                self.metrics
                    .insert(id.clone(), DataCollector::Gauge(gauge.clone()));
            }
            SetupMetrics::Counter(id, name, labels) => {
                let options = Opts::new(name, "help");
                let label_names: Vec<&str> = labels.iter().map(|s| &**s).collect();
                let gauge =
                    CounterVec::new(options, &label_names).expect("Couldn't create Counter");
                self.registry
                    .register(Box::new(gauge.clone()))
                    .expect("Couldn't register metric to prometheus");
                self.metrics
                    .insert(id.clone(), DataCollector::Counter(gauge.clone()));
            }
        }
    }
}

#[async_trait::async_trait]
impl Handler<SensorReading> for PrometheusCollector {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: SensorReading) {
        if let Some(dc) = self.metrics.get(&msg.id) {
            let lv: Vec<&str> = msg.labels.iter().map(|s| &**s).collect();
            match msg.reading {
                crate::msg::Value::Simple(v) => dc.set(&lv, v.into()),
                crate::msg::Value::Inc => dc.inc(&lv),
                crate::msg::Value::Dec => dc.dec(&lv),
            }
        } else {
            error!("Couldn't find collector '{}'", msg.id);
        }
    }
}

#[async_trait::async_trait]
impl Handler<EncodeData> for PrometheusCollector {
    async fn handle(&mut self, _ctx: &mut Context<Self>, msg: EncodeData) -> Result<String> {
        let metrics = self.registry.gather();
        let mut buffer = vec![];
        let encoder = TextEncoder::new();
        encoder.encode(&metrics, &mut buffer).unwrap();
        String::from_utf8(buffer).map_err(|e| e.into())
    }
}

// lazy_static! {
//     static ref A_INT_COUNTER: IntCounter =
//         register_int_counter!("A_int_counter", "foobar").unwrap();
//     static ref A_INT_COUNTER_VEC: IntCounterVec =
//         register_int_counter_vec!("A_int_counter_vec", "foobar", &["a", "b"]).unwrap();
//     static ref A_INT_GAUGE: IntGauge = register_int_gauge!("A_int_gauge", "foobar").unwrap();
//     static ref A_INT_GAUGE_VEC: IntGaugeVec =
//         register_int_gauge_vec!("A_int_gauge_vec", "foobar", &["a", "b"]).unwrap();
// }

// fn main() {
//     A_INT_COUNTER.inc();
//     A_INT_COUNTER.inc_by(10);
//     assert_eq!(A_INT_COUNTER.get(), 11);

//     A_INT_COUNTER_VEC.with_label_values(&["a", "b"]).inc_by(5);
//     assert_eq!(A_INT_COUNTER_VEC.with_label_values(&["a", "b"]).get(), 5);

//     A_INT_COUNTER_VEC.with_label_values(&["c", "d"]).inc();
//     assert_eq!(A_INT_COUNTER_VEC.with_label_values(&["c", "d"]).get(), 1);

//     A_INT_GAUGE.set(5);
//     assert_eq!(A_INT_GAUGE.get(), 5);
//     A_INT_GAUGE.dec();
//     assert_eq!(A_INT_GAUGE.get(), 4);
//     A_INT_GAUGE.add(2);
//     assert_eq!(A_INT_GAUGE.get(), 6);

//     A_INT_GAUGE_VEC.with_label_values(&["a", "b"]).set(10);
//     A_INT_GAUGE_VEC.with_label_values(&["a", "b"]).dec();
//     A_INT_GAUGE_VEC.with_label_values(&["a", "b"]).sub(2);
//     assert_eq!(A_INT_GAUGE_VEC.with_label_values(&["a", "b"]).get(), 7);
// }
