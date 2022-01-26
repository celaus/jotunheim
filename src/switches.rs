use crate::msg::SetupMetrics;
pub mod gpio;

pub mod http_handlers {
    #[derive(Clone)]
    pub struct SwitchHttpState {
        gpio: HashMap<String, SwitchAddr>,
    }

    use std::collections::HashMap;

    use anyhow::Result;
    use log::info;
    use tide::{Body, Request, Response, Server, StatusCode};
    use xactor::Actor;

    use crate::{
        config::Config,
        msg::{Switch, SwitchState},
        switches::gpio::GpioSwitch,
        SwitchAddr,
    };

    pub async fn switch(req: Request<SwitchHttpState>) -> tide::Result {
        let id = req.param("id")?;
        let on_off: i32 = req.param("value")?.parse()?;

        let state = req.state();
        let mut resp = Response::new(StatusCode::Ok);

        if let Some(gpio) = state.gpio.get(id) {
            info!("Triggering GPIO '{}'", id);
            if on_off == 0 {
                let _ = gpio.call(Switch::Off).await?;
            } else {
                let _ = gpio.call(Switch::On).await?;
            }
        } else {
            resp = Response::new(StatusCode::NotFound);
        }
        Ok(resp)
    }

    pub async fn switch_status(req: Request<SwitchHttpState>) -> tide::Result {
        let id = req.param("id")?;

        let state = req.state();
        let mut resp = Response::new(StatusCode::Ok);

        if let Some(gpio) = state.gpio.get(id) {
            let result = gpio.call(SwitchState {}).await? as u8;
            resp.set_body(Body::from_string(format!("{}", result)));
        } else {
            resp = Response::new(StatusCode::NotFound);
        }
        Ok(resp)
    }

    pub async fn init_and_setup(config: &Config) -> Result<Server<SwitchHttpState>> {
        let gpios = config.parsed_gpios().await;
        let mut switches = HashMap::new();
        if !gpios.is_empty() {
            info!("GPIO module active");
            let switches_actors: HashMap<String, GpioSwitch> = gpios
                .into_iter()
                .map(|(n, p)| (n.clone(), GpioSwitch::new(p, n)))
                .collect();
            info!(
                "GPIOs activated: {:?}",
                switches_actors.keys().collect::<Vec<&String>>()
            );
            for (name, actor) in switches_actors {
                let a = actor.start().await?;
                switches.insert(name, a);
            }
        }

        let mut app = tide::with_state(SwitchHttpState { gpio: switches });
        app.at("/:id/").get(switch_status);
        app.at("/:id/:value").get(switch);
        Ok(app)
    }
}
