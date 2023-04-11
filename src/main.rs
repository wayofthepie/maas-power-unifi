mod args;
pub mod config;
mod router;
pub mod unifi;

use args::Args;
use clap::Parser;
use config::read_config_file;
use reqwest::Client;
use router::{routes, AppState};
use unifi::Unifi;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let args = Args::parse();
    let config = &*Box::leak(Box::new(read_config_file(args.config_file).await?));
    let http_client = Client::builder()
        .cookie_store(true)
        .danger_accept_invalid_certs(true)
        .build()?;
    let client = Unifi::new(&config.url, http_client)?;
    let username = std::env::var("UNIFI_USERNAME").unwrap();
    let password = std::env::var("UNIFI_PASSWORD").unwrap();
    client.login(username, password).await?;
    let state = AppState { config, client };
    let app = routes(state);
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
    Ok(())
}
