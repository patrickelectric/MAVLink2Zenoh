#[macro_use]
extern crate lazy_static;
extern crate tracing;

use std::sync::Arc;

use mavlink::ardupilotmega::MavMessage;
use serde::{Deserialize, Serialize};
use zenoh::config::Config;
use zenoh::prelude::sync::*;

mod cli;
mod tasks;

use tracing::*;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MAVLinkMessage<T> {
    pub header: mavlink::MavHeader,
    pub message: T,
}

fn main() {
    env_logger::init();
    let mut task_master = tasks::TaskMaster::new();
    let args = &cli::App;
    let vehicle = Arc::new(mavlink::connect::<MavMessage>(&args.connect).unwrap());
    let heartbeat_vehicle = vehicle.clone();
    let sender_vehicle = vehicle.clone();
    debug!("Connection string: {}", &args.connect);

    let config = if let Some(config) = &args.config {
        Config::from_file(&config).unwrap()
    } else {
        Config::default()
    };
    let session = Arc::new(zenoh::open(config).res().unwrap());
    let publisher = session
        .declare_publisher(format!("{}/out", args.path))
        .res()
        .unwrap();
    let subscriber = session
        .declare_subscriber(format!("{}/in", args.path))
        .res()
        .unwrap();

    let (tx, mut rx1) = tokio::sync::broadcast::channel(1);
    let mut rx2 = tx.subscribe();

    task_master.spawn("MAVLink receiver".into(), async move {
        loop {
            match vehicle.recv() {
                Ok(msg) => {
                    debug!("Received MAVLink message from vehicle: {:?}", &msg);
                    let _ = tx.send(MAVLinkMessage {
                        header: msg.0,
                        message: msg.1,
                    });
                }
                Err(error) => {
                    error!("Error {:#?}", error);
                }
            }
        }
    });

    task_master.spawn("MAVLink Heartbeat Loop".into(), async move {
        while Err(tokio::sync::broadcast::error::TryRecvError::Closed) != rx1.try_recv() {
            debug!("Sending heartbeat..");
            let result = heartbeat_vehicle
                .send(&mavlink::MavHeader::default(), &heartbeat_message())
                .unwrap();
            debug!("M S2: {result:?}");
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    });

    task_master.spawn("MAVLink -> Zenoh".into(), async move {
        loop {
            match rx2.try_recv() {
                Ok(msg) => {
                    publisher
                        .put(json5::to_string(&msg).unwrap())
                        .res()
                        .unwrap();
                }
                Err(tokio::sync::broadcast::error::TryRecvError::Empty) => {}
                Err(tokio::sync::broadcast::error::TryRecvError::Closed) => {
                    break;
                }
                Err(error) => { trace!("{error:?}"); }
            };
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
    });

    task_master.spawn("Zenoh -> MAVLink".into(), async move {
        while let Ok(sample) = subscriber.recv_async().await {
            if let Ok(msg) = serde_json::Value::try_from(sample.value) {
                debug!("Received: {:?}", &msg);

                match serde_json::from_value::<MAVLinkMessage<mavlink::ardupilotmega::MavMessage>>(
                    msg.clone(),
                ) {
                    Ok(content) => {
                        let result = sender_vehicle.send(&content.header, &content.message);
                        debug!("Z->M: {result:?}");
                        continue;
                    }
                    Err(error) => {
                        error!("Failed to parse zenoh to MAVLink: {error:#?}");
                    }
                }

                match serde_json::from_value::<String>(msg) {
                    Ok(content) => match serde_json::from_str::<
                        MAVLinkMessage<mavlink::ardupilotmega::MavMessage>,
                    >(&content)
                    {
                        Ok(content) => {
                            let result = sender_vehicle.send(&content.header, &content.message);
                            debug!("Z-M: {result:?}");
                        }
                        Err(error) => error!("Failed to parse string to MAVLink: {error:#?}"),
                    },
                    Err(error) => error!("Failed to parse zenoh to string: {error:#?}"),
                }
            }
        }
    });
}

pub fn heartbeat_message() -> mavlink::ardupilotmega::MavMessage {
    mavlink::ardupilotmega::MavMessage::HEARTBEAT(mavlink::ardupilotmega::HEARTBEAT_DATA {
        custom_mode: 0,
        mavtype: mavlink::ardupilotmega::MavType::MAV_TYPE_GENERIC,
        autopilot: mavlink::ardupilotmega::MavAutopilot::MAV_AUTOPILOT_GENERIC,
        base_mode: mavlink::ardupilotmega::MavModeFlag::empty(),
        system_status: mavlink::ardupilotmega::MavState::MAV_STATE_STANDBY,
        mavlink_version: 0x3,
    })
}
