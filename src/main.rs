mod unifi;

use reqwest::Client;
use unifi::Unifi;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let username = std::env::var("UNIFI_USERNAME")?;
    let password = std::env::var("UNIFI_PASSWORD")?;
    let client = Client::builder()
        .cookie_store(true)
        .danger_accept_invalid_certs(true)
        .build()?;
    let unifi_client = Unifi::new("https://localhost:8443", client)?;
    unifi_client.login(username, password).await?;
    let response = unifi_client.devices().await?;
    for device in response.data {
        for port in device.port_table {
            println!("{port:?}");
        }
    }
    Ok(())
}
