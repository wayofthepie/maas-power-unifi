use super::{
    client::UnifiClient,
    models::{AuthData, Device, PoeMode, UnifiResponse},
};
use async_trait::async_trait;
use hyper::{header::CONTENT_TYPE, Method};
use reqwest::{Client, Url};

use serde_json::json;

#[derive(Clone, Debug)]
pub struct UnifiSelfHostedClient {
    base_url: Url,
    client: Client,
}

impl UnifiSelfHostedClient {
    pub fn new<S: AsRef<str>>(base_url: S, client: Client) -> anyhow::Result<Self> {
        let url = Url::parse(base_url.as_ref())?;
        Ok(Self {
            base_url: url,
            client,
        })
    }

    async fn power(
        &self,
        poe_mode: PoeMode,
        device_id: &str,
        port_number: usize,
    ) -> anyhow::Result<UnifiResponse<()>> {
        let url = self.base_url.join("/api/s/default/rest/device/")?;
        let url = url.join(device_id)?;
        let body = serde_json::to_string(
            &json!({"port_overrides":[{"port_idx":port_number,"poe_mode":poe_mode}]}),
        )?;
        let response = self
            .client
            .request(Method::POST, url)
            .header(CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await?;
        response.error_for_status()?;
        Ok(UnifiResponse {
            data: (),
            ..Default::default()
        })
    }
}

#[async_trait]
impl UnifiClient for UnifiSelfHostedClient {
    async fn login(&self, username: &str, password: &str) -> anyhow::Result<()> {
        let auth_data = AuthData::new(username.into(), password.into());
        let auth_data_json = serde_json::to_string(&auth_data)?;
        let url = self.base_url.join("/api/login")?;
        let response = self
            .client
            .request(Method::POST, url)
            .header(CONTENT_TYPE, "application/json")
            .body(auth_data_json)
            .send()
            .await?;
        Ok(response.error_for_status().map(|_| ())?)
    }

    async fn devices(&self) -> anyhow::Result<UnifiResponse<Vec<Device>>> {
        let url = self.base_url.join("/api/s/default/stat/device")?;
        let response = self
            .client
            .request(Method::GET, url)
            .header(CONTENT_TYPE, "application/json")
            .send()
            .await?;
        let response = response.error_for_status()?;
        Ok(response.json::<UnifiResponse<Vec<Device>>>().await?)
    }

    async fn power_on(
        &self,
        device_id: &str,
        port_number: usize,
    ) -> anyhow::Result<UnifiResponse<()>> {
        self.power(PoeMode::Auto, device_id, port_number).await
    }

    async fn power_off(
        &self,
        device_id: &str,
        port_number: usize,
    ) -> anyhow::Result<UnifiResponse<()>> {
        self.power(PoeMode::Off, device_id, port_number).await
    }
}

#[cfg(test)]
mod test {
    use crate::unifi::models::{Meta, PoeMode};

    use super::{Device, UnifiClient, UnifiResponse, UnifiSelfHostedClient};
    use serde_json::json;
    use wiremock::{
        matchers::{body_json, method, path},
        Mock, MockServer, ResponseTemplate,
    };

    const UNIFI_DEVICE_ID: &str = "device-id";

    #[test]
    fn should_give_error_if_base_url_fails_to_parse() {
        let url = "http//localhost";
        let r_client = reqwest::Client::new();
        let client = UnifiSelfHostedClient::new(url, r_client);
        assert!(client.is_err());
    }

    #[tokio::test]
    async fn should_login() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/login"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;
        let unifi_client =
            UnifiSelfHostedClient::new(mock_server.uri(), reqwest::Client::new()).unwrap();
        let response = unifi_client.login("", "").await;
        assert!(response.is_ok(), "{:?}", response);
    }

    #[tokio::test]
    async fn should_list_devices() {
        let mock_server = MockServer::start().await;
        let response = UnifiResponse::<Vec<Device>>::default();
        Mock::given(method("GET"))
            .and(path("/api/s/default/stat/device"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response))
            .mount(&mock_server)
            .await;
        let unifi_client =
            UnifiSelfHostedClient::new(mock_server.uri(), reqwest::Client::new()).unwrap();
        let response = unifi_client.devices().await;
        assert!(response.is_ok(), "{:?}", response);
    }

    #[tokio::test]
    async fn should_power_on_machine() {
        let mock_server = MockServer::start().await;
        let port_number = 1;
        let response = UnifiResponse::<Vec<Device>> {
            meta: Meta {
                rc: "ok".to_owned(),
            },
            ..Default::default()
        };
        Mock::given(method("POST"))
            .and(path(format!(
                "/api/s/default/rest/device/{}",
                UNIFI_DEVICE_ID
            )))
            .and(body_json(
                json!({"port_overrides":[{"port_idx":port_number,"poe_mode":PoeMode::Auto}]}),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(response))
            .mount(&mock_server)
            .await;
        let unifi_client =
            UnifiSelfHostedClient::new(mock_server.uri(), reqwest::Client::new()).unwrap();
        unifi_client
            .power_on(UNIFI_DEVICE_ID, port_number)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn should_power_off_machine() {
        let mock_server = MockServer::start().await;
        let port_number = 1;
        let response = UnifiResponse::<Vec<Device>> {
            meta: Meta {
                rc: "ok".to_owned(),
            },
            ..Default::default()
        };
        Mock::given(method("POST"))
            .and(path(format!(
                "/api/s/default/rest/device/{}",
                UNIFI_DEVICE_ID
            )))
            .and(body_json(
                json!({"port_overrides":[{"port_idx":port_number,"poe_mode":PoeMode::Off}]}),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(response))
            .mount(&mock_server)
            .await;
        let unifi_client =
            UnifiSelfHostedClient::new(mock_server.uri(), reqwest::Client::new()).unwrap();
        unifi_client
            .power_off(UNIFI_DEVICE_ID, port_number)
            .await
            .unwrap();
    }
}
