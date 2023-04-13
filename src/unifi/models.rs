use serde::{Deserialize, Serialize};

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
    pub mac: String,
    pub device_id: String,
    pub port_table: Vec<Port>,
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
