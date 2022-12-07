use crate::msg::DeviceControl;
use anyhow::Result;
use futures_util::future::join_all;
use log::{error, info};
use serde::Deserialize;
use surf::http::Method;
use tide::{Request, Response, Server, StatusCode};
use xactor::{Actor, Addr, Handler};

use crate::config::Config;

pub struct ActorEndpoints<T> {
    actors: Vec<Addr<T>>,
}

impl<T> Clone for ActorEndpoints<T> {
    fn clone(&self) -> Self {
        Self {
            actors: self.actors.clone(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct Generic {
    p: String,
}

pub async fn send_device_control<T: 'static + Actor + Handler<DeviceControl>>(
    mut req: Request<ActorEndpoints<T>>,
) -> tide::Result {
    info!("HTTP: {}/{:?}", req.method(), req.url().query());
    let msg = if req.method() == Method::Post || req.method() == Method::Put {
        DeviceControl {
            payload: req.body_bytes().await?,
        }
    } else {
        let q: Generic = req.query()?;
        DeviceControl {
            payload: q.p.into_bytes(),
        }
    };
    info!("Payload: {}", String::from_utf8_lossy(&msg.payload));
    let state = req.state();
    let resp = Response::new(StatusCode::Ok);
    join_all(state.actors.iter().map(|a| a.call(msg.clone()))).await;
    Ok(resp)
}

pub async fn register_actors<T: 'static + Actor + Handler<DeviceControl>>(
    actors: Vec<Addr<T>>,
) -> Result<Server<ActorEndpoints<T>>> {
    let mut app = tide::with_state(ActorEndpoints { actors });
    app.at("/").all(send_device_control);
    Ok(app)
}
