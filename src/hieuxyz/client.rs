#![allow(non_snake_case)]
use tokio::sync::{Mutex, RwLock, mpsc};
use std::sync::Arc;
use futures_util::future::BoxFuture;
use serde_json::Value;

use crate::hieuxyz::gateway::discord_websocket::{DiscordWebSocket, DiscordWebSocketOptions, GatewayEvent};
use crate::hieuxyz::gateway::entities::{identify::ClientProperties, types::{PresenceUpdatePayload, DiscordUser, UserFlags}};
use crate::hieuxyz::rpc::{hieuxyz_rpc::HieuxyzRPC, image_service::ImageService};
use crate::hieuxyz::utils::logger::logger;

/// Configuration options for initializing the Client.
#[derive(Clone, Default)]
pub struct ClientOptions {
    /// Your Discord user token.
    pub token: String,
    /// (Optional) Base URL of the image proxy service.
    pub apiBaseUrl: Option<String>,
    /// (Optional) If true, the client will attempt to reconnect even after a normal close (code 1000).
    /// Defaults to false.
    pub alwaysReconnect: Option<bool>,
    /// (Optional) Client properties to send to Discord gateway.
    /// Used for client spoofing.
    pub properties: Option<ClientProperties>,
    /// (Optional) The timeout in milliseconds for the initial gateway connection.
    /// Defaults to 30000 (30 seconds).
    pub connectionTimeout: Option<u64>,
}

/// The main Client class for interacting with Discord Rich Presence.
/// This is the starting point for creating and managing your RPC state.
///
/// # Example
/// ```no_run
/// use hieuxyz_rpc::{Client, ClientOptions};
///
/// #[tokio::main]
/// async fn main() {
///    let client = Client::new(ClientOptions {
///        token: "TOKEN".to_string(),
///        ..Default::default()
///    });
///    client.run().await;
/// }
/// ```
pub struct Client {
    _token: String,
    imageService: ImageService,
    /// List of all RPC instances managed by this client.
    rpcs: Arc<RwLock<Vec<Arc<RwLock<HieuxyzRPC>>>>>,
    websocket: Arc<Mutex<Option<DiscordWebSocket>>>,
    /// The default RPC instance.
    /// Use this to set your main Rich Presence state details.
    pub rpc: Arc<RwLock<HieuxyzRPC>>,
    /// Information about the logged-in user.
    /// Populated after run() resolves.
    pub user: Arc<RwLock<Option<DiscordUser>>>,
    /// Receiver for gateway events (Ready, Resumed)
    gateway_observer_rx: Arc<Mutex<mpsc::Receiver<GatewayEvent>>>,
}

impl Client {
    /// Create a new Client instance.
    ///
    /// # Arguments
    ///
    /// * `options` - Options to configure the client.
    ///
    /// # Panics
    ///
    /// Panics if no token is provided in the options.
    pub fn new(options: ClientOptions) -> Arc<Self> {
        if options.token.is_empty() {
             panic!("Tokens are required to connect to Discord.");
        }
        let imageService = ImageService::new(options.apiBaseUrl);
        let rpcs_vec = Arc::new(RwLock::new(Vec::new()));
        let (notify_tx, notify_rx) = mpsc::channel(10);
        let ws_options = DiscordWebSocketOptions {
             alwaysReconnect: options.alwaysReconnect.unwrap_or(false),
             properties: options.properties,
             connectionTimeout: options.connectionTimeout.unwrap_or(30000),
        };
        let websocket = DiscordWebSocket::new(options.token.clone(), ws_options, notify_tx);
        let rpc = HieuxyzRPC::new(imageService.clone(), Arc::new(|| Box::pin(async {})));
        
        let client = Arc::new(Self {
            _token: options.token,
            imageService,
            rpcs: rpcs_vec,
            websocket: Arc::new(Mutex::new(Some(websocket))),
            rpc: Arc::new(RwLock::new(rpc)),
            user: Arc::new(RwLock::new(None)),
            gateway_observer_rx: Arc::new(Mutex::new(notify_rx)),
        });
        client.printAbout();
        client
    }

    /// Connect to Discord Gateway and prepare the client for RPC updates.
    /// This method must be called before sending any Rich Presence updates.
    ///
    /// # Returns
    ///
    /// Returns the `DiscordUser` object containing profile information when ready.
    pub async fn run(self: &Arc<Self>) -> DiscordUser {
        let me_weak = Arc::downgrade(self);
        let cb = Arc::new(move || {
             let weak = me_weak.clone();
             Box::pin(async move { 
                 if let Some(strong) = weak.upgrade() {
                     strong.sendAllActivities().await; 
                 }
             }) as BoxFuture<'static, ()>
        });
        {
             let mut default_rpc = self.rpc.write().await;
             *default_rpc = HieuxyzRPC::new(self.imageService.clone(), cb.clone());
             self.rpcs.write().await.push(self.rpc.clone());
        }

        let mut ws_guard = self.websocket.lock().await;
        if let Some(w) = ws_guard.as_mut() {
             w.connect();
             logger::info("Waiting for Discord session to be ready...");
             let mut rx = self.gateway_observer_rx.lock().await;
             let user = loop {
                 match rx.recv().await {
                     Some(GatewayEvent::Ready(u)) => break u,
                     Some(_) => continue, 
                     None => panic!("Gateway channel closed unexpectedly"),
                 }
             };
             *self.user.write().await = Some(user.clone());
             self.logUserProfile(&user);
             drop(rx);
        } else {
            panic!("WebSocket not initialized");
        }
        let self_clone = self.clone();
        tokio::spawn(async move {
            let mut rx = self_clone.gateway_observer_rx.lock().await;
            while let Some(event) = rx.recv().await {
                match event {
                    GatewayEvent::Ready(_) | GatewayEvent::Resumed => {
                        logger::info("Connection restored/ready. Re-sending Rich Presence...");
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        self_clone.sendAllActivities().await;
                    },
                    _ => {}
                }
            }
        });

        logger::info("Client is ready to send Rich Presence updates.");
        self.user.read().await.clone().unwrap()
    }

    /// Create a new RPC instance.
    /// Use this if you want to display multiple activities simultaneously (Multi-RPC).
    ///
    /// # Returns
    ///
    /// A new RPC builder instance wrapped in Arc<RwLock>.
    pub async fn createRPC(self: &Arc<Self>) -> Arc<RwLock<HieuxyzRPC>> {
        let me_weak = Arc::downgrade(self);
        let cb = Arc::new(move || {
             let weak = me_weak.clone();
             Box::pin(async move { 
                 if let Some(strong) = weak.upgrade() {
                     strong.sendAllActivities().await; 
                 }
             }) as BoxFuture<'static, ()>
        });

        let rpc = HieuxyzRPC::new(self.imageService.clone(), cb);
        let rpc_arc = Arc::new(RwLock::new(rpc));
        self.rpcs.write().await.push(rpc_arc.clone());
        rpc_arc
    }

    /// Removes an RPC instance and cleans up its resources.
    ///
    /// # Arguments
    ///
    /// * `rpcInstance` - The RPC instance to remove.
    pub async fn removeRPC(&self, rpcInstance: Arc<RwLock<HieuxyzRPC>>) {
        let mut list = self.rpcs.write().await;
        if let Some(pos) = list.iter().position(|x| Arc::ptr_eq(x, &rpcInstance)) {
            let rpc = list.remove(pos);
            rpc.write().await.destroy();
        }
        drop(list); 
        self.sendAllActivities().await;
    }

    /// Aggregates activities from all RPC instances and sends them to Discord.
    /// Uses concurrent processing for asset resolution.
    pub async fn sendAllActivities(&self) {
        let rpcs = self.rpcs.read().await;
        let mut potential = Vec::new();
        let mut status = "online".to_string();

        for rpc_lock in rpcs.iter() {
            let rpc = rpc_lock.read().await;
            potential.push(rpc.buildActivity().await);
        }
        for rpc_lock in rpcs.iter().rev() {
            let rpc = rpc_lock.read().await;
            let s = rpc.get_currentStatus();
             if !s.is_empty() { status = s; break; }
        }

        let activities: Vec<_> = potential.into_iter().filter_map(|a| a).collect();
        let payload = PresenceUpdatePayload {
            since: 0,
            activities,
            status,
            afk: true
        };

        if let Some(ws) = self.websocket.lock().await.as_ref() {
            ws.sendActivity(payload).await;
        }
    }

    /// Close the connection to Discord Gateway.
    ///
    /// # Arguments
    ///
    /// * `force` - If true, the client closes permanently and will not reconnect.
    pub async fn close(&self, force: bool) {
         let list = self.rpcs.write().await;
         for rpc in list.iter() { rpc.write().await.destroy(); }
         if let Some(ws) = self.websocket.lock().await.as_ref() {
             ws.close(force).await;
         }
    }

    fn printAbout(&self) {
        println!(r#"
  _     _
 | |__ (_) ___ _   ___  ___   _ ______
 | '_ \| |/ _ \ | | \ \/ / | | |_  /
 | | | | |  __/ |_| |>  <| |_| |/ /
 |_| |_|_|\___|\__,_/_/\_\\__, /___|
                          |___/
  hieuxyz_rpc v0.0.3
  A powerful Discord Rich Presence library.
  Developed by: hieuxyz
        "#);
    }

    fn formatFlags(flags: u64) -> String {
        let mut names = Vec::new();
        if flags & UserFlags::STAFF != 0 { names.push("Staff"); }
        if flags & UserFlags::PARTNER != 0 { names.push("Partner"); }
        if flags & UserFlags::HYPESQUAD != 0 { names.push("HypeSquad"); }
        if flags & UserFlags::BUG_HUNTER_LEVEL_1 != 0 { names.push("BugHunter I"); }
        if flags & UserFlags::HYPESQUAD_ONLINE_HOUSE_1 != 0 { names.push("Bravery"); }
        if flags & UserFlags::HYPESQUAD_ONLINE_HOUSE_2 != 0 { names.push("Brilliance"); }
        if flags & UserFlags::HYPESQUAD_ONLINE_HOUSE_3 != 0 { names.push("Balance"); }
        if flags & UserFlags::PREMIUM_EARLY_SUPPORTER != 0 { names.push("EarlySupporter"); }
        if flags & UserFlags::BUG_HUNTER_LEVEL_2 != 0 { names.push("BugHunter II"); }
        if flags & UserFlags::VERIFIED_DEVELOPER != 0 { names.push("VerifiedDev"); }
        if flags & UserFlags::CERTIFIED_MODERATOR != 0 { names.push("CertifiedMod"); }
        if flags & UserFlags::ACTIVE_DEVELOPER != 0 { names.push("ActiveDev"); }
        let list_str = if names.is_empty() { "None".to_string() } else { names.join(", ") };
        format!("{} \x1b[36m[{}]\x1b[0m", flags, list_str)
    }

    fn printDynamicTree(&self, val: &Value, prefix: &str, _key_name: Option<&str>) {
        match val {
            Value::Object(map) => {
                 let entries: Vec<_> = map.iter().collect();
                 for (i, (k, v)) in entries.iter().enumerate() {
                     let is_last = i == entries.len() - 1;
                     let connector = if is_last { "└── " } else { "├── " };
                     let child_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });

                     if v.is_object() && !v.is_array() {
                         println!("{}{}\x1b[1m{}\x1b[0m", prefix, connector, k);
                         self.printDynamicTree(v, &child_prefix, None);
                     } else if v.is_array() {
                         let display = if v.as_array().unwrap().is_empty() { "[Array(0)]".to_string() } else {
                             let arr = v.as_array().unwrap();
                             if arr.len() > 0 && !arr[0].is_object() {
                                 let items: Vec<String> = arr.iter().map(|i| i.to_string()).collect();
                                 format!("[ {} ]", items.join(", "))
                             } else {
                                 format!("[Array({})]", arr.len())
                             }
                         };
                         println!("{}{}{}: {}", prefix, connector, k, display);
                     } else {
                         let display = if *k == "email" || *k == "phone" {
                             if !v.is_null() { "\x1b[90m<Hidden>\x1b[0m".to_string() } else { "null".to_string() }
                         } else if *k == "flags" || *k == "public_flags" {
                             if let Some(f) = v.as_u64() { Self::formatFlags(f) } else { "0".to_string() }
                         } else if *k == "premium_type" {
                             match v.as_u64() {
                                 Some(0) => "0 (\x1b[32mNone\x1b[0m)".to_string(),
                                 Some(1) => "1 (\x1b[32mClassic\x1b[0m)".to_string(),
                                 Some(2) => "2 (\x1b[32mNitro\x1b[0m)".to_string(),
                                 Some(3) => "3 (\x1b[32mBasic\x1b[0m)".to_string(),
                                 _ => format!("{} (\x1b[32mUnknown\x1b[0m)", v)
                             }
                         } else if *k == "avatar" || *k == "banner" {
                             if v.is_null() { "null".to_string() } else {
                                 let s = v.as_str().unwrap_or("");
                                 format!("\"{}\"", s)
                             }
                         } else if *k == "banner_color" || *k == "accent_color" {
                              format!("\x1b[33m{}\x1b[0m", v)
                         } else {
                             if v.is_string() { format!("\"\x1b[32m{}\x1b[0m\"", v.as_str().unwrap()) }
                             else if v.is_boolean() { if v.as_bool().unwrap() { "\x1b[32mtrue\x1b[0m".to_string() } else { "\x1b[31mfalse\x1b[0m".to_string() } }
                             else if v.is_number() { format!("\x1b[33m{}\x1b[0m", v) }
                             else { "null".to_string() }
                         };
                         println!("{}{}{}: {}", prefix, connector, k, display);
                     }
                 }
            },
            _ => {}
        }
    }

    fn logUserProfile(&self, user: &DiscordUser) {
        logger::info("-> User Data:");
        let val = serde_json::to_value(user).unwrap();
        self.printDynamicTree(&val, "", None);
    }
}
