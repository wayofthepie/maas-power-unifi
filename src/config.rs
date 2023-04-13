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

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Machine {
    pub maas_id: String,
    pub port_id: usize,
}

impl Config {
    /// Given the ID of a machine in MaaS, returns the MAC address of the associated
    /// unifi device that manages it.
    pub fn owning_device_mac(&self, maas_id: &str) -> Option<String> {
        let maybe_device = self.devices.iter().find(|device| {
            device
                .machines
                .iter()
                .any(|machine| machine.maas_id == maas_id)
        });
        maybe_device.map(|device| device.mac.clone())
    }

    /// Gets the machine that corresponds to the given MaaS system ID.
    pub fn machine(&self, maas_id: &str) -> Option<Machine> {
        self.devices
            .iter()
            .find(|device| {
                device
                    .machines
                    .iter()
                    .any(|machine| machine.maas_id == maas_id)
            })
            .and_then(|d| d.machines.first())
            .cloned()
    }
}

pub async fn read_config_file(config_file: PathBuf) -> anyhow::Result<Config> {
    let config_str = tokio::fs::read_to_string(config_file).await?;
    let config = toml::from_str::<Config>(&config_str)?;
    Ok(config)
}

#[cfg(test)]
mod test {
    use crate::config::Machine;

    use super::read_config_file;
    use std::path::PathBuf;

    const MAAS_ID: &str = "maas_id";
    const PORT_ID: usize = 2;
    const UNIFI_DEVICE_MAC: &str = "xx:xx:xx:xx:xx:xx";

    #[tokio::test]
    async fn should_return_mac_addr_of_unifi_device() {
        let mut config_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        config_path.push("resources/example.toml");
        let config = read_config_file(config_path).await.unwrap();
        assert!(config.owning_device_mac(MAAS_ID).is_some());
        assert_eq!(config.owning_device_mac(MAAS_ID).unwrap(), UNIFI_DEVICE_MAC);
    }

    #[tokio::test]
    async fn should_get_machine_matching_id() {
        let expected_machine = Machine {
            maas_id: MAAS_ID.to_owned(),
            port_id: PORT_ID,
        };
        let mut config_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        config_path.push("resources/example.toml");
        let config = read_config_file(config_path).await.unwrap();
        assert!(config.machine(MAAS_ID).is_some());
        assert_eq!(config.machine(MAAS_ID).unwrap(), expected_machine);
    }
}
