use hieuxyz_rpc::{Client, ClientOptions, logger};
use std::env;
use dotenv::dotenv;
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;
#[tokio::main(flavor = "current_thread")]
async fn main() {
    std::env::set_var("MALLOC_CONF", "background_thread:true,dirty_decay_ms:0,muzzy_decay_ms:0");
    dotenv().ok();
    let token = env::var("DISCORD_USER_TOKEN").unwrap_or_default();
    if token.is_empty() {
        logger::error("Token not found in .env file. Please set DISCORD_USER_TOKEN.");
        return;
    }
    let client = Client::new(ClientOptions {
        token,
        alwaysReconnect: Some(true),
        apiBaseUrl: None,
        properties: None,
        connectionTimeout: None,
    });
    client.run().await;
    {
        let mut rpc = client.rpc.write().await;
        rpc.setName("a")
           .setDetails("b")
           .setState("c")
           .setType(0)
           .setStatus("dnd")
           .setTimestamps(Some(1768204694273), None)
           .addButton("View Github", "https://github.com/hieuxyz00");
    }
    {
        let rpc = client.rpc.read().await;
        rpc.build().await;
    }
    tokio::signal::ctrl_c().await.unwrap();
    logger::info("SIGINT received. Closing connection...");
    client.close(true).await;
}
