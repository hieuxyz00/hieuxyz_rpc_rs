#![allow(non_snake_case)]
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use std::time::{Duration, Instant};
use serde_json::json;
use flate2::read::ZlibDecoder;
use std::io::Read;
use url::Url;

use crate::hieuxyz::utils::logger::logger;
use crate::hieuxyz::gateway::entities::{opcode::OpCode, types::*, identify::*};

/// Manages the WebSocket connection to the Discord Gateway.
pub struct DiscordWebSocket {
    token: String,
    options: DiscordWebSocketOptions,
    cmd_tx: mpsc::Sender<WebSocketCommand>, 
    pub user: Option<DiscordUser>,
    /// A channel that sends the user info once the READY event is received.
    pub readyPromise: mpsc::Receiver<DiscordUser>,
}

#[derive(Clone)]
pub struct DiscordWebSocketOptions {
    pub alwaysReconnect: bool,
    pub properties: Option<ClientProperties>,
    pub connectionTimeout: u64,
}

enum WebSocketCommand {
    SendJson(serde_json::Value),
    Close(bool),
}

impl DiscordWebSocket {
    pub fn new(token: String, options: DiscordWebSocketOptions) -> Self {
        let (tx, _) = mpsc::channel(1);
        let (_, rx) = mpsc::channel(1);
        
        if !Self::isTokenValid(&token) {
             panic!("Invalid token provided.");
        }

        Self {
            token,
            options,
            cmd_tx: tx,
            user: None,
            readyPromise: rx,
        }
    }

    fn isTokenValid(token: &str) -> bool {
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() == 3 {
            return !parts[0].is_empty() && !parts[1].is_empty() && parts[2].len() >= 20;
        }
        false
    }

    /// Initiates the WebSocket connection in a background task.
    pub fn connect(&mut self) {
        let (cmd_tx, mut cmd_rx) = mpsc::channel(32);
        let (ready_tx, ready_rx) = mpsc::channel(1);
        self.cmd_tx = cmd_tx;
        self.readyPromise = ready_rx;
        let token = self.token.clone();
        let options = self.options.clone();

        tokio::spawn(async move {
            let mut session_id: Option<String> = None;
            let mut resume_gateway_url: Option<String> = None;
            let mut sequence: Option<u64> = None;
            let mut permanent_close = false;
            let mut is_reconnecting = false;

            loop {
                if permanent_close { break; }
                
                if is_reconnecting {
                     logger::info("Connection attempt aborted: reconnection already in progress.");
                }
                
                is_reconnecting = true;
                
                let url_str = resume_gateway_url.clone()
                    .unwrap_or_else(|| "wss://gateway.discord.gg/?v=10&encoding=json".to_string());
                
                logger::info(&format!("Attempting to connect to {}...", url_str));

                let url = Url::parse(&url_str).unwrap();
                let connect_future = connect_async(url.as_str());
                let ws_result = tokio::time::timeout(
                    Duration::from_millis(options.connectionTimeout),
                    connect_future
                ).await;

                match ws_result {
                    Ok(Ok((ws_stream, _))) => {
                        logger::info(&format!("Successfully connected to Discord Gateway at {}.", url_str));
                        is_reconnecting = false;
                        let (mut write, mut read) = ws_stream.split();
                        let (hb_tx, mut hb_rx) = mpsc::channel::<()>(1);
                        let mut hb_task: Option<JoinHandle<()>> = None;
                        let mut last_ack_received = Instant::now();
                        let mut last_heartbeat_sent = Instant::now();
                        let mut has_sent_first_heartbeat = false;
                        let (socket_out_tx, mut socket_out_rx) = mpsc::channel::<Message>(32);
                        let write_handle = tokio::spawn(async move {
                            while let Some(msg) = socket_out_rx.recv().await {
                                if write.send(msg).await.is_err() { break; }
                            }
                        });

                        loop {
                            tokio::select! {
                                msg_opt = read.next() => {
                                    match msg_opt {
                                        Some(Ok(message)) => {
                                            let payload_json: Option<String> = match message {
                                                Message::Binary(bin) => {
                                                    let mut decoder = ZlibDecoder::new(&bin[..]);
                                                    let mut s = String::new();
                                                    match decoder.read_to_string(&mut s) {
                                                        Ok(_) => Some(s),
                                                        Err(e) => {
                                                            logger::error(&format!("Zlib decode error: {}", e));
                                                            None
                                                        }
                                                    }
                                                },
                                                Message::Text(text) => Some(text.to_string()),
                                                Message::Close(frame) => {
                                                    let code = frame.as_ref().map(|f| f.code.into()).unwrap_or(1000);
                                                    let reason = frame.as_ref().map(|f| f.reason.to_string()).unwrap_or_default();
                                                    logger::warn(&format!("Connection closed: {} - {}", code, reason));
                                                    
                                                    if code == 4004 || code == 4999 {
                                                        session_id = None; sequence = None; resume_gateway_url = None;
                                                    }
                                                    
                                                    let fatal_codes = vec![4004, 4010, 4011, 4013, 4014];
                                                    if fatal_codes.contains(&code) {
                                                         logger::error(&format!("Fatal WebSocket error received (code: {}). Will not reconnect.", code));
                                                         permanent_close = true;
                                                    } else if !options.alwaysReconnect && code == 1000 {
                                                         logger::info("Not attempting to reconnect based on close code and client options.");
                                                         permanent_close = true;
                                                    }
                                                    break;
                                                },
                                                _ => None,
                                            };

                                            if let Some(json_str) = payload_json {
                                                if let Ok(payload) = serde_json::from_str::<GatewayPayload>(&json_str) {
                                                    if let Some(s) = payload.s { sequence = Some(s); }

                                                    match payload.op {
                                                        OpCode::HELLO => {
                                                            let d = payload.d.unwrap();
                                                            let hb_interval = d["heartbeat_interval"].as_u64().unwrap_or(41250);
                                                            
                                                            logger::info(&format!("Received HELLO. Setting heartbeat interval to {}ms.", hb_interval));
                                                            
                                                            last_ack_received = Instant::now(); 
                                                            has_sent_first_heartbeat = false;
                                                            if let Some(t) = hb_task { t.abort(); }
                                                            let hb_tx_clone = hb_tx.clone();
                                                            
                                                            hb_task = Some(tokio::spawn(async move {
                                                                let jitter = (hb_interval as f64 * rand::random::<f64>()) as u64;
                                                                tokio::time::sleep(Duration::from_millis(jitter)).await;
                                                                if hb_tx_clone.send(()).await.is_err() { return; }
                                                                let mut interval = tokio::time::interval(Duration::from_millis(hb_interval));
                                                                interval.tick().await; 
                                                                loop {
                                                                    interval.tick().await;
                                                                    if hb_tx_clone.send(()).await.is_err() { break; }
                                                                }
                                                            }));

                                                            if session_id.is_some() && sequence.is_some() {
                                                                let resume = json!({
                                                                    "op": 6,
                                                                    "d": {
                                                                        "token": token,
                                                                        "session_id": session_id.clone().unwrap(),
                                                                        "seq": sequence.unwrap()
                                                                    }
                                                                });
                                                                let _ = socket_out_tx.send(Message::Text(resume.to_string().into())).await;
                                                                logger::info("Resume payload sent.");
                                                            } else {
                                                                let identify = getIdentifyPayload(&token, options.properties.clone());
                                                                let json = json!({ "op": 2, "d": identify });
                                                                let _ = socket_out_tx.send(Message::Text(json.to_string().into())).await;
                                                                logger::info("Identify payload sent.");
                                                            }
                                                        },
                                                        OpCode::DISPATCH => {
                                                            if let Some(t) = payload.t.as_deref() {
                                                                if t == "READY" {
                                                                    let d = payload.d.unwrap();
                                                                    session_id = d["session_id"].as_str().map(|s| s.to_string());
                                                                    resume_gateway_url = d["resume_gateway_url"].as_str().map(|s| s.to_string());
                                                                    
                                                                    logger::info(&format!("Session READY. Session ID: {}. Resume URL set.", session_id.as_ref().unwrap()));
                                                                    
                                                                    let user: DiscordUser = serde_json::from_value(d["user"].clone()).unwrap();
                                                                    let _ = ready_tx.send(user).await;
                                                                } else if t == "RESUMED" {
                                                                    logger::info("The session has been successfully resumed.");
                                                                }
                                                            }
                                                        },
                                                        OpCode::HEARTBEAT_ACK => {
                                                            logger::info("Heartbeat acknowledged.");
                                                            last_ack_received = Instant::now();
                                                        },
                                                        OpCode::HEARTBEAT => {
                                                             let hb = json!({ "op": 1, "d": sequence });
                                                             let _ = socket_out_tx.send(Message::Text(hb.to_string().into())).await;
                                                        },
                                                        OpCode::INVALID_SESSION => {
                                                            let resumable = payload.d.and_then(|v| v.as_bool()).unwrap_or(false);
                                                            logger::warn(&format!("Received INVALID_SESSION. Resumable: {}", resumable));
                                                            if !resumable { 
                                                                session_id = None; sequence = None; 
                                                                let _ = socket_out_tx.send(Message::Close(Some(tokio_tungstenite::tungstenite::protocol::CloseFrame { code: 4004.into(), reason: "Invalid session".into() }))).await;
                                                            } else {
                                                                let _ = socket_out_tx.send(Message::Close(Some(tokio_tungstenite::tungstenite::protocol::CloseFrame { code: 4000.into(), reason: "Resuming".into() }))).await;
                                                            }
                                                            break; 
                                                        },
                                                        OpCode::RECONNECT => {
                                                            logger::info("Gateway requested RECONNECT. Closing to reconnect and resume.");
                                                            let _ = socket_out_tx.send(Message::Close(Some(tokio_tungstenite::tungstenite::protocol::CloseFrame { code: 4000.into(), reason: "Reconnect".into() }))).await;
                                                            break;
                                                        },
                                                        _ => {}
                                                    }
                                                }
                                            }
                                        },
                                        Some(Err(e)) => {
                                            logger::error(&format!("WebSocket Error: {}", e));
                                            break;
                                        },
                                        None => {
                                            logger::warn("Connection closed.");
                                            break;
                                        }
                                    }
                                },

                                _ = hb_rx.recv() => {
                                    if has_sent_first_heartbeat {
                                        if last_ack_received < last_heartbeat_sent {
                                            logger::warn("Heartbeat ACK missing. Connection is zombie. Terminating to resume...");
                                            break;
                                        }
                                    }

                                    let hb = json!({ "op": 1, "d": sequence });
                                    let _ = socket_out_tx.send(Message::Text(hb.to_string().into())).await;
                                    last_heartbeat_sent = Instant::now();
                                    has_sent_first_heartbeat = true;
                                    let seq_str = match sequence {
                                        Some(s) => s.to_string(),
                                        None => "null".to_string(),
                                    };
                                    logger::info(&format!("Heartbeat sent with sequence {}.", seq_str));
                                },

                                cmd = cmd_rx.recv() => {
                                    match cmd {
                                        Some(WebSocketCommand::SendJson(val)) => {
                                            let _ = socket_out_tx.send(Message::Text(val.to_string().into())).await;
                                        },
                                        Some(WebSocketCommand::Close(force)) => {
                                            if force { 
                                                logger::info("Forcing permanent closure. Reconnects will be disabled.");
                                                permanent_close = true; 
                                            } else {
                                                logger::info("Closing connection manually...");
                                            }
                                            let _ = socket_out_tx.send(Message::Close(Some(tokio_tungstenite::tungstenite::protocol::CloseFrame { code: 1000.into(), reason: "Client initiated closure".into() }))).await;
                                            break;
                                        },
                                        None => break,
                                    }
                                }
                            }
                        }

                        if let Some(t) = hb_task { t.abort(); }
                        write_handle.abort();
                    },
                    Ok(Err(e)) => {
                        logger::error(&format!("WebSocket connection failed: {}", e));
                    },
                    Err(_) => {
                        logger::error("Connection timed out. Terminating connection attempt.");
                    }
                }

                if permanent_close { break; }
                logger::info("Attempting to reconnect in 5 seconds...");
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        });
    }

    /// Sends a presence update payload to the gateway.
    pub async fn sendActivity(&self, presence: PresenceUpdatePayload) {
        let json = json!({ "op": OpCode::PRESENCE_UPDATE, "d": presence });
        let _ = self.cmd_tx.send(WebSocketCommand::SendJson(json)).await;
        logger::info("Presence update sent.");
    }

    /// Closes the connection.
    pub async fn close(&self, force: bool) {
        let _ = self.cmd_tx.send(WebSocketCommand::Close(force)).await;
    }
}