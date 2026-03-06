#![allow(non_snake_case)]
use reqwest::{Client, multipart};
use serde::Deserialize;
use std::path::Path;
use tokio::fs::File;
use tokio_util::codec::{BytesCodec, FramedRead};
use url::Url;
use crate::hieuxyz::utils::logger::logger;

#[derive(Debug, Deserialize)]
pub struct DiscordAsset {
    pub id: String,
    pub r#type: u8,
    pub name: String,
}

#[derive(Deserialize)]
struct ImageResponse {
    status: u16,
    id: Option<String>,
    message_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct UploadResult {
    pub id: String,
    pub message_id: Option<String>,
}

/// Service handling image uploads, external proxying, and asset renewal.
#[derive(Clone)]
pub struct ImageService {
    apiClient: Client,
    apiBaseUrl: String,
}

impl ImageService {
    pub fn new(apiBaseUrl: Option<String>) -> Self {
        Self {
            apiClient: Client::new(),
            apiBaseUrl: apiBaseUrl.unwrap_or_else(|| "https://rpc.hieuxyz.fun".to_string()),
        }
    }

    /// Requests a proxy URL for an external image.
    pub async fn getExternalUrl(&self, url: &str) -> Option<UploadResult> {
        let mut target_url = match Url::parse(&format!("{}/image", self.apiBaseUrl)) {
            Ok(u) => u,
            Err(e) => {
                logger::error(&format!("Invalid API Base URL: {}", e));
                return None;
            }
        };
        target_url.query_pairs_mut().append_pair("url", url);
        match self.apiClient.get(target_url).send().await {
            Ok(res) => {
                if let Ok(bytes) = res.bytes().await {
                    if let Ok(data) = serde_json::from_slice::<ImageResponse>(&bytes) {
                        if data.status == 200 && data.id.is_some() {
                            return Some(UploadResult {
                                id: data.id.unwrap(),
                                message_id: None
                            });
                        }
                    }
                }
            },
            Err(e) => logger::error(&format!("Unable to get external proxy URL for {}: {}", url, e)),
        }
        None
    }

    /// Uploads a local file to the image service.
    pub async fn uploadImage(&self, filePath: &str, fileName: &str) -> Option<UploadResult> {
        if !Path::new(filePath).exists() {
            logger::error(&format!("File not found at path: {}", filePath));
            return None;
        }
        let file = match File::open(filePath).await {
            Ok(f) => f,
            Err(e) => {
                logger::error(&format!("Unable to open file: {}", e));
                return None;
            }
        };
        let stream = FramedRead::new(file, BytesCodec::new());
        let file_body = reqwest::Body::wrap_stream(stream);
        let form = multipart::Form::new()
            .part("file", multipart::Part::stream(file_body).file_name(fileName.to_string()))
            .text("file_name", fileName.to_string());

        match self.apiClient.post(format!("{}/upload", self.apiBaseUrl))
            .multipart(form)
            .send().await {
            Ok(res) => {
                 if let Ok(bytes) = res.bytes().await {
                    if let Ok(data) = serde_json::from_slice::<ImageResponse>(&bytes) {
                        if data.status == 200 && data.id.is_some() {
                            return Some(UploadResult {
                                id: data.id.unwrap(),
                                message_id: data.message_id
                            });
                        }
                    }
                }
            },
            Err(e) => logger::error(&format!("Unable to upload image {}: {}", fileName, e)),
        }
        None
    }

    /// Renews a signed URL asset if it is expiring.
    pub async fn renewImage(&self, assetId: &str) -> Option<String> {
        match self.apiClient.post(format!("{}/renew", self.apiBaseUrl))
            .json(&serde_json::json!({ "asset_id": assetId }))
            .send().await {
            Ok(res) => {
                 if let Ok(bytes) = res.bytes().await {
                    if let Ok(data) = serde_json::from_slice::<ImageResponse>(&bytes) {
                        if data.status == 200 && data.id.is_some() {
                            logger::info(&format!("Successfully renewed asset: {}", assetId));
                            return data.id;
                        }
                    }
                }
            },
            Err(e) => logger::error(&format!("Failed to renew asset {}: {}", assetId, e)),
        }
        None
    }

    /// Fetches all assets associated with a Discord Application ID.
    pub async fn fetchApplicationAssets(&self, applicationId: &str) -> Vec<DiscordAsset> {
        let url = format!("https://discord.com/api/v9/oauth2/applications/{}/assets", applicationId);
        match self.apiClient.get(&url).send().await {
            Ok(res) => {
                if let Ok(bytes) = res.bytes().await {
                    serde_json::from_slice::<Vec<DiscordAsset>>(&bytes).unwrap_or_else(|_| Vec::new())
                } else {
                    Vec::new()
                }
            },
            Err(e) => {
                logger::error(&format!("Failed to fetch assets for application {}: {}. Ensure App ID is correct.", applicationId, e));
                Vec::new()
            }
        }
    }
}