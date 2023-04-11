use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub url: String,
    pub devices: Vec<Device>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Device {
    pub mac: String,
    pub machines: Vec<Machine>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Machine {
    pub maas_id: String,
    pub port_id: usize,
}

pub async fn read_config_file(config_file: PathBuf) -> anyhow::Result<Config> {
    let config_str = tokio::fs::read_to_string(config_file).await?;
    let config = toml::from_str::<Config>(&config_str)?;
    Ok(config)
}
