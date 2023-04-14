use crate::{
    config::Config,
    unifi::{client::UnifiError, handler::UnifiHandler, models::PowerStatus},
};
use async_trait::async_trait;
use axum::{
    extract::{FromRef, FromRequestParts},
    response::{IntoResponse, Response},
    routing::{get, post},
    Extension, Json, Router,
};
use http::{request::Parts, StatusCode};
use serde_json::json;
use tracing::instrument;

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
            AppError::Power(UnifiError::FailedToConvertSystemId(error)) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to convert system_id to string: {error}"),
            ),
        };
        let body = Json(json!({
            "error": error_message,
        }));
        (status, body).into_response()
    }
}

const SYSTEM_ID: &str = "system_id";

struct ExtractSystemId(String);

#[async_trait]
impl<S> FromRequestParts<S> for ExtractSystemId
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        if let Some(system_id) = parts.headers.get(SYSTEM_ID) {
            let system_id = system_id.to_str().map_err(|_e| {
                (
                    StatusCode::BAD_REQUEST,
                    "Failed to convert system_id header to a string!",
                )
            })?;
            Ok(ExtractSystemId(system_id.to_owned()))
        } else {
            Err((StatusCode::BAD_REQUEST, "`system_id` header is missing"))
        }
    }
}

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/power-status", get(power_status))
        .route("/power-on", post(power_on))
        //.route("/power-off", post(power_off))
        .layer(Extension(state))
}

#[instrument(skip(handler))]
async fn power_status(
    Extension(AppState { config, handler }): Extension<AppState>,
    ExtractSystemId(system_id): ExtractSystemId,
) -> Result<Json<PowerStatus>, AppError> {
    let mac = config
        .owning_device_mac(&system_id)
        .ok_or(UnifiError::DeviceNotFound(system_id.to_owned()))?;
    let machine = config
        .machine(&system_id)
        .ok_or(UnifiError::MachineNotFound(system_id.to_string()))?;
    let device_id = handler.device_id(&mac).await?;
    let device = handler.device(&device_id).await?;
    device
        .power_status(machine.port_id)
        .map(Json)
        .ok_or(UnifiError::DeviceNotFound("".to_owned()).into())
}

async fn power_on(
    Extension(AppState { config, handler }): Extension<AppState>,
    ExtractSystemId(system_id): ExtractSystemId,
) -> Result<(), AppError> {
    let mac = config
        .owning_device_mac(&system_id)
        .ok_or(UnifiError::DeviceNotFound(system_id.to_owned()))?;
    let machine = config
        .machine(&system_id)
        .ok_or(UnifiError::MachineNotFound(system_id.to_string()))?;
    let device_id = handler.device_id(&mac).await?;
    Ok(handler.power_on(&device_id, machine.port_id).await?)
}

#[cfg(test)]
mod test {
    use crate::{
        config::{self, Config, Machine},
        router::{routes, AppState, PowerStatus},
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
    use std::str::FromStr;
    use tower::ServiceExt;

    const UNIFI_DEVICE_MAC: &str = "00-00-00-00-00-00";
    const MAAS_SYSTEM_ID_HEADER: &str = "system_id";
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

    #[tokio::test]
    async fn should_power_on() {
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
            .method(Method::POST)
            .uri("/power-on")
            .header(MAAS_SYSTEM_ID_HEADER, MAAS_SYSTEM_ID)
            .body(Body::empty())
            .unwrap();
        let response = routes(state).oneshot(request).await.unwrap();
        assert_eq!(response.status(), 200);
    }
}
