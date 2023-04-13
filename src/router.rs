use std::time::Duration;

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
use tower_http::trace::TraceLayer;
use tracing::Span;

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
    PowerOn(UnifiError),
}

impl From<UnifiError> for AppError {
    fn from(inner: UnifiError) -> Self {
        AppError::PowerOn(inner)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::PowerOn(UnifiError::DeviceListError(s)) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to list devices, error: {s}"),
            ),
            AppError::PowerOn(UnifiError::FailedToConstructUrl(s)) => {
                (StatusCode::UNPROCESSABLE_ENTITY, s)
            }
            AppError::PowerOn(UnifiError::MissingSystemId) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "System ID was not found in MaaS request.".to_owned(),
            ),
            AppError::PowerOn(UnifiError::DeviceNotFound(mac)) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Device with mac address {mac} was not found!"),
            ),
            AppError::PowerOn(UnifiError::MachineNotFound(system_id)) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Machine with system id {system_id} was not found!"),
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
        .route("/", get(power_status))
        .layer(Extension(state))
        .layer(TraceLayer::new_for_http().on_response(
            |response: &Response, _latency: Duration, span: &Span| {
                span.record("status_code", &tracing::field::display(response.status()));
                tracing::debug!("{response:?}");
            },
        ))
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
        .get("system_id")
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
                .unwrap();
            if let Some(PoeMode::Auto) = port.poe_mode {
                return Ok(Json(PowerStatus {
                    status: "running".to_owned(),
                }));
            }
        }
    }

    Err(UnifiError::DeviceNotFound("".to_owned()).into())
}
