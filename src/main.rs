mod args;
pub mod config;
mod router;
pub mod unifi;

use args::Args;
use clap::Parser;
use config::read_config_file;
use reqwest::Client;
use router::{routes, AppState};
use tracing::Level;
use tracing_subscriber::{filter, prelude::*};
use unifi::{client::UnifiClient, handler::UnifiHandler, self_hosted::UnifiSelfHostedClient};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let filter = filter::Targets::new()
        // Enable the `INFO` level for anything in `my_crate`
        .with_target("maas_power_unifi", Level::DEBUG);
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(filter)
        .init();

    let args = Args::parse();
    let config = &*Box::leak(Box::new(read_config_file(args.config_file).await?));
    let http_client = Client::builder()
        .cookie_store(true)
        .danger_accept_invalid_certs(true)
        .build()?;
    let client = Box::new(UnifiSelfHostedClient::new(&config.url, http_client)?);
    let username = std::env::var("UNIFI_USERNAME").unwrap();
    let password = std::env::var("UNIFI_PASSWORD").unwrap();
    client.login(&username, &password).await?;
    let handler = UnifiHandler { client };
    let state = AppState { config, handler };
    let app = routes(state);
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
    Ok(())
}
