use std::fmt::Display;

use mac_address::MacAddress;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct PowerStatus {
    pub status: String,
}

#[derive(Serialize, Deserialize)]
pub struct AuthData {
    username: String,
    password: String,
}

impl AuthData {
    pub fn new(username: String, password: String) -> Self {
        Self { username, password }
    }
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct UnifiResponse<T> {
    pub meta: Meta,
    pub data: T,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Meta {
    pub rc: String,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Device {
    pub mac: MacAddress,
    pub device_id: DeviceId,
    pub port_table: Vec<Port>,
}

impl Device {
    pub fn power_status(&self, port_id: usize) -> Option<PowerStatus> {
        self.port_table
            .iter()
            .find(|port| port.port_idx == port_id)
            .and_then(|port| match port.poe_mode {
                Some(PoeMode::Auto) => Some(PowerStatus {
                    status: "running".to_owned(),
                }),
                Some(PoeMode::Off) => Some(PowerStatus {
                    status: "stopped".to_owned(),
                }),
                _ => None,
            })
    }
}

#[derive(Serialize, Deserialize, Default, Debug, PartialEq)]
pub struct DeviceId(String);

impl DeviceId {
    pub fn new<S: Into<String>>(device_id_str: S) -> Self {
        Self(device_id_str.into())
    }
}

impl Display for DeviceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Port {
    pub port_idx: usize,
    pub poe_mode: Option<PoeMode>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum PoeMode {
    Auto,
    Off,
}
