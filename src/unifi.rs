use hyper::{header::CONTENT_TYPE, Method};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct AuthData {
    username: String,
    password: String,
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

#[derive(Debug)]
pub enum UnifiError {
    MissingSystemId,
    MachineNotFound(String),
    DeviceListError(String),
    FailedToConstructUrl(String),
    DeviceNotFound(String),
}

#[derive(Clone, Debug)]
pub struct Unifi {
    base_url: Url,
    client: Client,
}

impl Unifi {
    pub fn new<S: AsRef<str>>(base_url: S, client: Client) -> anyhow::Result<Self> {
        let url = Url::parse(base_url.as_ref())?;
        Ok(Self {
            base_url: url,
            client,
        })
    }

    pub async fn login<S: Into<String>>(&self, username: S, password: S) -> anyhow::Result<()> {
        let auth_data = AuthData {
            username: username.into(),
            password: password.into(),
        };
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

    pub async fn devices(&self) -> anyhow::Result<UnifiResponse<Vec<Device>>> {
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

    pub async fn power_status(&self) -> anyhow::Result<UnifiResponse<Vec<Device>>> {
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
}

#[cfg(test)]
mod test {
    use crate::unifi::{Device, UnifiResponse};

    use super::Unifi;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

    #[test]
    fn should_give_error_if_base_url_fails_to_parse() {
        let url = "http//localhost";
        let r_client = reqwest::Client::new();
        let client = Unifi::new(url, r_client);
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
        let unifi_client = Unifi::new(mock_server.uri(), reqwest::Client::new()).unwrap();
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
        let unifi_client = Unifi::new(mock_server.uri(), reqwest::Client::new()).unwrap();
        let response = unifi_client.devices().await;
        assert!(response.is_ok(), "{:?}", response);
    }
}
