use crate::{
    config::Config,
    unifi::{
        client::UnifiError,
        handler::UnifiHandler,
        models::{PoeMode, PowerStatus},
    },
};
use axum::{
    extract::FromRef,
    response::{IntoResponse, Response},
    routing::{get, post},
    Extension, Json, Router,
};
use http::{HeaderMap, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::instrument;

const MAAS_SYSTEM_ID_HEADER: &str = "system_id";

#[derive(Clone)]
pub struct AppState {
    pub config: &'static Config,
    pub handler: UnifiHandler,
}

impl FromRef<AppState> for UnifiHandler {
    fn from_ref(state: &AppState) -> UnifiHandler {
        state.handler.clone()
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
            AppError::Power(UnifiError::FailedToPowerOn(device_id)) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to power on a port on the device {device_id}!"),
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
        //.route("/power-on", post(power_on))
        //.route("/power-off", post(power_off))
        .layer(Extension(state))
}

#[instrument(skip(handler))]
async fn power_status(
    Extension(AppState { config, handler }): Extension<AppState>,
    headers: HeaderMap,
) -> Result<Json<PowerStatus>, AppError> {
    let system_id = headers
        .get(MAAS_SYSTEM_ID_HEADER)
        .ok_or(UnifiError::MissingSystemId)?
        .to_str()
        .unwrap();
    let mac = config
        .owning_device_mac(system_id)
        .ok_or(UnifiError::DeviceNotFound(system_id.to_owned()))?;
    let machine = config
        .machine(system_id)
        .ok_or(UnifiError::MachineNotFound(system_id.to_string()))?;
    let device_id = handler.device_id(&mac).await?;
    let device = handler.device(&device_id).await?;
    device
        .power_status(machine.port_id)
        .map(Json)
        .ok_or(UnifiError::DeviceNotFound("".to_owned()).into())
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use crate::{
        config::{self, Config, Machine},
        router::{routes, AppState, PowerStatus, MAAS_SYSTEM_ID_HEADER},
        unifi::{
            self,
            client::UnifiClient,
            handler::UnifiHandler,
            models::{DeviceId, Meta, PoeMode, Port, UnifiResponse},
        },
    };
    use async_trait::async_trait;
    use http::{Method, Request};
    use hyper::{body, Body};
    use mac_address::MacAddress;
    use tower::ServiceExt;

    const UNIFI_DEVICE_MAC: &str = "00-00-00-00-00-00";
    const MAAS_SYSTEM_ID: &str = "system-id";
    const MACHINE_PORT: usize = 1;

    #[derive(Clone)]
    struct FakeUnifi {}

    #[async_trait]
    impl UnifiClient for FakeUnifi {
        async fn login(&self, _: &str, _: &str) -> anyhow::Result<()> {
            Ok(())
        }

        async fn devices(&self) -> anyhow::Result<UnifiResponse<Vec<unifi::models::Device>>> {
            Ok(UnifiResponse {
                meta: Meta { rc: "".to_owned() },
                data: vec![unifi::models::Device {
                    mac: MacAddress::from_str(UNIFI_DEVICE_MAC).unwrap(),
                    device_id: DeviceId::new(MAAS_SYSTEM_ID),
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

    #[tokio::test]
    async fn should_get_power_status() {
        let config = Box::leak(Box::new(Config {
            url: "".to_owned(),
            devices: vec![config::Device {
                mac: MacAddress::from_str(UNIFI_DEVICE_MAC).unwrap(),
                machines: vec![Machine {
                    maas_id: MAAS_SYSTEM_ID.to_owned(),
                    port_id: MACHINE_PORT,
                }],
            }],
        }));
        let client = Box::new(FakeUnifi {});
        let handler = UnifiHandler { client };
        let state = AppState { config, handler };
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
