use hieuxyz_rpc::{Client, ClientOptions, logger};
use std::env;
use tokio::time::{sleep, Duration};
use std::time::{SystemTime, UNIX_EPOCH};
use dotenv::dotenv;

#[tokio::main]
async fn main() {
    dotenv().ok();
    let token = env::var("DISCORD_USER_TOKEN").unwrap_or_default();
    if token.is_empty() {
        logger::error("Token not found in .env file. Please set DISCORD_USER_TOKEN.");
        return;
    }
    let client = Client::new(ClientOptions {
        token,
        alwaysReconnect: None,
        apiBaseUrl: None,
        properties: None,
        connectionTimeout: None,
    });
    client.run().await;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
    {
        let mut rpc = client.rpc.write().await;
        rpc.setName("Visual Studio Code")
           .setDetails("Writing Rust code")
           .setState("Workspace: hieuxyz_rpc")
           .setPlatform("desktop")
           .setType(0) // Playing
           .setTimestamps(Some(now), None)
           .setParty(1, 5, None)
           .setApplicationId("914622396630175855")
           .setLargeImage("rust_file", None)
           .setSmallImage("vs_2026", None)
           .setButtons(vec![
               ("View on GitHub".to_string(), "https://github.com/hieuxyz00/hieuxyz_rpc_rs".to_string()),
               ("View on Crates.io".to_string(), "https://crates.io/crates/hieuxyz_rpc".to_string())
           ]);
    }
    {
        let rpc = client.rpc.read().await;
        rpc.build().await;
    }

    logger::info("Initial Rich Presence has been updated. Check your Discord profile.");
    logger::info("An update will occur in 15 seconds. Press Ctrl+C to exit.");

    let client_clone = client.clone();
    tokio::spawn(async move {
        sleep(Duration::from_secs(15)).await;
        logger::info("Updating RPC details dynamically...");
        {
            let mut rpc = client_clone.rpc.write().await;
            rpc.setDetails("Idle").setState("Waiting for compile...").setParty(2, 5, None);
        }
        {
             let rpc = client_clone.rpc.read().await;
             rpc.updateRPC().await;
        }
        logger::info("RPC has been dynamically updated. Check your Discord profile again!");
    });

    tokio::signal::ctrl_c().await.unwrap();
    logger::info("SIGINT received. Closing connection...");
    client.close(true).await;
}