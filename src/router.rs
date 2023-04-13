use crate::{
    config::Config,
    unifi::{PoeMode, UnifiClient, UnifiError},
};
use axum::{
    extract::FromRef,
    response::{IntoResponse, Response},
    routing::get,
    Extension, Json, Router,
};
use http::{HeaderMap, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::json;

const MAAS_SYSTEM_ID_HEADER: &str = "system_id";

#[derive(Clone)]
pub struct AppState {
    pub config: &'static Config,
    pub client: Box<dyn UnifiClient + Send + Sync>,
}

impl FromRef<AppState> for Box<dyn UnifiClient> {
    fn from_ref(state: &AppState) -> Box<dyn UnifiClient> {
        state.client.clone()
    }
}

enum AppError {
    Power(UnifiError),
}

impl From<UnifiError> for AppError {
    fn from(inner: UnifiError) -> Self {
        AppError::Power(inner)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::Power(UnifiError::DeviceListError(s)) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to list devices, error: {s}"),
            ),
            AppError::Power(UnifiError::FailedToConstructUrl(s)) => {
                (StatusCode::UNPROCESSABLE_ENTITY, s)
            }
            AppError::Power(UnifiError::MissingSystemId) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "System ID was not found in MaaS request.".to_owned(),
            ),
            AppError::Power(UnifiError::DeviceNotFound(mac)) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Device with mac address {mac} was not found!"),
            ),
            AppError::Power(UnifiError::MachineNotFound(system_id)) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Machine with system id {system_id} was not found!"),
            ),
            AppError::Power(UnifiError::MachinePortIdIncorrect(port_id)) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Found no machine on port {port_id}!"),
            ),
        };
        let body = Json(json!({
            "error": error_message,
        }));
        (status, body).into_response()
    }
}

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/power-status", get(power_status))
        .layer(Extension(state))
}

#[derive(Serialize, Deserialize)]
struct PowerStatus {
    status: String,
}

async fn power_status(
    Extension(AppState { config, client }): Extension<AppState>,
    headers: HeaderMap,
) -> Result<Json<PowerStatus>, AppError> {
    let system_id = headers
        .get(MAAS_SYSTEM_ID_HEADER)
        .ok_or(UnifiError::MissingSystemId)?
        .to_str()
        .unwrap();
    for managed_device in config.devices.iter() {
        if let Some(machine) = managed_device
            .machines
            .iter()
            .find(|machine| machine.maas_id == system_id)
        {
            let response = client
                .devices()
                .await
                .map_err(|e| UnifiError::DeviceListError(e.to_string()))?;
            let device = response
                .data
                .iter()
                .find(|device| device.mac == managed_device.mac)
                .ok_or(UnifiError::DeviceNotFound(managed_device.mac.to_owned()))?;
            let port = device
                .port_table
                .iter()
                .find(|port| port.port_idx == machine.port_id)
                .ok_or(UnifiError::MachinePortIdIncorrect(machine.port_id))?;
            if let Some(PoeMode::Auto) = port.poe_mode {
                return Ok(Json(PowerStatus {
                    status: "running".to_owned(),
                }));
            }
        }
    }

    Err(UnifiError::DeviceNotFound("".to_owned()).into())
}

#[cfg(test)]
mod test {
    use crate::{
        config::{self, Config, Machine},
        router::{routes, AppState, PowerStatus, MAAS_SYSTEM_ID_HEADER},
        unifi::{self, Meta, PoeMode, Port, UnifiClient, UnifiResponse},
    };
    use async_trait::async_trait;
    use http::{Method, Request};
    use hyper::{body, Body};
    use tower::ServiceExt;

    const UNIFI_DEVICE_MAC: &str = "00-00-00-00-00-00";
    const MAAS_SYSTEM_ID: &str = "system-id";
    const MACHINE_PORT: usize = 1;

    #[derive(Clone)]
    struct FakeUnifi {}

    #[async_trait]
    impl UnifiClient for FakeUnifi {
        async fn login(&self, _: &str, _: &str) -> anyhow::Result<()> {
            unimplemented!()
        }

        async fn devices(&self) -> anyhow::Result<UnifiResponse<Vec<unifi::Device>>> {
            Ok(UnifiResponse {
                meta: Meta { rc: "".to_owned() },
                data: vec![unifi::Device {
                    mac: UNIFI_DEVICE_MAC.to_owned(),
                    device_id: "".to_owned(),
                    port_table: vec![Port {
                        port_idx: MACHINE_PORT,
                        poe_mode: Some(PoeMode::Auto),
                    }],
                }],
            })
        }
    }

    #[tokio::test]
    async fn should_get_power_status() {
        let config = Box::leak(Box::new(Config {
            url: "".to_owned(),
            devices: vec![config::Device {
                mac: UNIFI_DEVICE_MAC.to_owned(),
                machines: vec![Machine {
                    maas_id: MAAS_SYSTEM_ID.to_owned(),
                    port_id: MACHINE_PORT,
                }],
            }],
        }));
        let client = Box::new(FakeUnifi {});
        let state = AppState { config, client };
        let request = Request::builder()
            .method(Method::GET)
            .uri("/power-status")
            .header(MAAS_SYSTEM_ID_HEADER, MAAS_SYSTEM_ID)
            .body(Body::empty())
            .unwrap();
        let mut response = routes(state).oneshot(request).await.unwrap();
        let body = response.body_mut();
        let power_status =
            serde_json::from_slice::<PowerStatus>(&body::to_bytes(body).await.unwrap()).unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(power_status.status, "running");
    }
}
