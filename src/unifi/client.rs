use super::models::{Device, UnifiResponse};
use async_trait::async_trait;
use dyn_clone::DynClone;

#[derive(Debug)]
pub enum UnifiError {
    MissingSystemId,
    MachineNotFound(String),
    DeviceListError(String),
    FailedToConstructUrl(String),
    DeviceNotFound(String),
    MachinePortIdIncorrect(usize),
    FailedToPowerOn(String),
    FailedToConvertSystemId(String),
}

#[async_trait]
pub trait UnifiClient: DynClone {
    async fn login(&self, username: &str, password: &str) -> anyhow::Result<()>;

    async fn devices(&self) -> anyhow::Result<UnifiResponse<Vec<Device>>>;

    async fn power_on(
        &self,
        device_id: &str,
        port_number: usize,
    ) -> anyhow::Result<UnifiResponse<()>>;

    async fn power_off(
        &self,
        device_id: &str,
        port_number: usize,
    ) -> anyhow::Result<UnifiResponse<()>>;
}
dyn_clone::clone_trait_object!(UnifiClient);
