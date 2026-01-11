//! # hieuxyz_rpc
//!
//! `hieuxyz_rpc` is a Discord Rich Presence library ported from the TypeScript project `@hieuxyz/rpc`.
//! It allows you to control the RPC status of a Discord user account via the Gateway.
//!
//! ## Example
//!
//! ```no_run
//! use hieuxyz_rpc::{Client, ClientOptions};
//!
//! #[tokio::main]
//! async fn main() {
//!     let client = Client::new(ClientOptions {
//!         token: "YOUR_TOKEN".to_string(),
//!         alwaysReconnect: Some(true),
//!         apiBaseUrl: None,
//!         properties: None,
//!         connectionTimeout: None,
//!     });
//!     
//!     client.run().await;
//!     
//!     {
//!         let mut rpc = client.rpc.write().await;
//!         rpc.setName("Hello World");
//!     }
//!     client.rpc.read().await.build().await;
//! }
//! ```
//!
//! See more details at [README](https://github.com/hieuxyz00/hieuxyz_rpc_rs)

pub mod hieuxyz;

pub use hieuxyz::client::{Client, ClientOptions};
pub use hieuxyz::gateway::discord_websocket::DiscordWebSocket;
pub use hieuxyz::rpc::hieuxyz_rpc::HieuxyzRPC;
pub use hieuxyz::rpc::image_service::ImageService;
pub use hieuxyz::rpc::rpc_image::{RpcImage, DiscordImage, ExternalImage, LocalImage, RawImage, ApplicationImage};
pub use hieuxyz::utils::logger::logger;
pub use hieuxyz::gateway::entities::types::*;
pub use hieuxyz::gateway::entities::identify::ClientProperties;