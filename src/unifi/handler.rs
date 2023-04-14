use super::{
    client::{UnifiClient, UnifiError},
    models::{Device, DeviceId},
};
use mac_address::MacAddress;

#[derive(Clone)]
pub struct UnifiHandler {
    pub client: Box<dyn UnifiClient + Send + Sync>,
}

impl UnifiHandler {
    pub async fn power_on(&self, device_id: &DeviceId, port_id: usize) -> Result<(), UnifiError> {
        self.client
            .power_on(&device_id.to_string(), port_id)
            .await
            .map(|_| ())
            .map_err(|e| UnifiError::FailedToPowerOn(e.to_string()))
    }

    // Given a device mac, return the ID in the unifi controller
    pub async fn device_id(&self, device_mac: &MacAddress) -> Result<DeviceId, UnifiError> {
        let response = self
            .client
            .devices()
            .await
            .map_err(|e| UnifiError::DeviceListError(e.to_string()))?;
        let device = response
            .data
            .into_iter()
            .find(|device| device.mac == *device_mac)
            .ok_or(UnifiError::DeviceNotFound(device_mac.to_string()))?;
        Ok(device.device_id)
    }

    pub async fn device(&self, device_id: &DeviceId) -> Result<Device, UnifiError> {
        self.client
            .devices()
            .await
            .map_err(|e| UnifiError::DeviceListError(e.to_string()))?
            .data
            .into_iter()
            .find(|device| device.device_id == *device_id)
            .ok_or(UnifiError::DeviceNotFound(device_id.to_string()))
    }
}

#[cfg(test)]
mod test {
    use crate::unifi::{
        self,
        client::UnifiClient,
        handler::UnifiHandler,
        models::{DeviceId, Meta, PoeMode, Port, UnifiResponse},
    };
    use async_trait::async_trait;
    use mac_address::MacAddress;

    const UNIFI_DEVICE_MAC: [u8; 6] = [00, 00, 00, 00, 00, 00];
    const UNIFI_DEVICE_ID: &str = "device-id";
    const MACHINE_PORT: usize = 1;

    #[derive(Clone)]
    struct FakeUnifiClient {}

    #[derive(Clone)]
    struct FailingUnifiClient {}

    #[async_trait]
    impl UnifiClient for FakeUnifiClient {
        async fn login(&self, _: &str, _: &str) -> anyhow::Result<()> {
            Ok(())
        }

        async fn devices(&self) -> anyhow::Result<UnifiResponse<Vec<unifi::models::Device>>> {
            Ok(UnifiResponse {
                meta: Meta { rc: "".to_owned() },
                data: vec![unifi::models::Device {
                    mac: MacAddress::from(UNIFI_DEVICE_MAC),
                    device_id: DeviceId::new(UNIFI_DEVICE_ID),
                    port_table: vec![Port {
                        port_idx: MACHINE_PORT,
                        poe_mode: Some(PoeMode::Auto),
                    }],
                }],
            })
        }

        async fn power_on(&self, _: &str, _: usize) -> anyhow::Result<UnifiResponse<()>> {
            Ok(UnifiResponse {
                data: (),
                ..Default::default()
            })
        }

        async fn power_off(&self, _: &str, _: usize) -> anyhow::Result<UnifiResponse<()>> {
            Ok(UnifiResponse {
                data: (),
                ..Default::default()
            })
        }
    }

    #[async_trait]
    impl UnifiClient for FailingUnifiClient {
        async fn login(&self, _: &str, _: &str) -> anyhow::Result<()> {
            Ok(())
        }

        async fn devices(&self) -> anyhow::Result<UnifiResponse<Vec<unifi::models::Device>>> {
            Ok(UnifiResponse {
                meta: Meta { rc: "".to_owned() },
                data: vec![unifi::models::Device {
                    mac: MacAddress::from(UNIFI_DEVICE_MAC),
                    device_id: DeviceId::new(UNIFI_DEVICE_ID),
                    port_table: vec![Port {
                        port_idx: MACHINE_PORT,
                        poe_mode: Some(PoeMode::Auto),
                    }],
                }],
            })
        }

        async fn power_on(&self, _: &str, _: usize) -> anyhow::Result<UnifiResponse<()>> {
            Err(anyhow::anyhow!("failed"))
        }

        async fn power_off(&self, _: &str, _: usize) -> anyhow::Result<UnifiResponse<()>> {
            Err(anyhow::anyhow!("failed"))
        }
    }

    #[tokio::test]
    async fn should_get_device_id() {
        let client = Box::new(FakeUnifiClient {});
        let handler = UnifiHandler { client };
        let device_id = handler
            .device_id(&MacAddress::from(UNIFI_DEVICE_MAC))
            .await
            .unwrap();
        assert_eq!(device_id, DeviceId::new(UNIFI_DEVICE_ID));
    }

    #[tokio::test]
    async fn should_get_device() {
        let client = Box::new(FakeUnifiClient {});
        let handler = UnifiHandler { client };
        let device = handler
            .device(&DeviceId::new(UNIFI_DEVICE_ID))
            .await
            .unwrap();
        assert_eq!(device.device_id, DeviceId::new(UNIFI_DEVICE_ID));
    }

    #[tokio::test]
    async fn should_power_on() {
        let client = Box::new(FakeUnifiClient {});
        let handler = UnifiHandler { client };
        handler
            .power_on(&DeviceId::new(UNIFI_DEVICE_ID), MACHINE_PORT)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn should_error_if_power_on_fails() {
        let client = Box::new(FailingUnifiClient {});
        let handler = UnifiHandler { client };
        let result = handler
            .power_on(&DeviceId::new(UNIFI_DEVICE_ID), MACHINE_PORT)
            .await;
        assert!(result.is_err());
    }
}
