#![allow(non_snake_case)]
use super::image_service::ImageService;
use std::path::Path;
use url::Url;

/// Represents the source of an image used in Rich Presence.
#[derive(Clone, Debug)]
pub enum RpcImage {
    /// An image already hosted on Discord (e.g., `mp:attachments/...`).
    Discord(String),
    /// An external URL (will be proxied).
    External(String),
    /// A local file path. It will be uploaded.
    Local { path: String, name: String },
    /// A raw asset key string.
    Raw(String),
    /// An asset name defined in the Discord Developer Portal for the specific Application ID.
    Application(String),
}

/// Helper struct to create a `RpcImage::Discord`.
pub struct DiscordImage; 
impl DiscordImage { pub fn new(key: &str) -> RpcImage { RpcImage::Discord(key.to_string()) } }

/// Helper struct to create a `RpcImage::External`.
pub struct ExternalImage; 
impl ExternalImage { pub fn new(url: &str) -> RpcImage { RpcImage::External(url.to_string()) } }

/// Helper struct to create a `RpcImage::Local`.
pub struct LocalImage; 
impl LocalImage { 
    /// Creates a local image reference.
    /// If `fileName` is None, the filename from `filePath` is used.
    pub fn new(filePath: &str, fileName: Option<&str>) -> RpcImage { 
        let name = fileName.map(|s| s.to_string()).unwrap_or_else(|| {
             Path::new(filePath).file_name().unwrap_or_default().to_string_lossy().to_string()
        });
        RpcImage::Local { path: filePath.to_string(), name } 
    } 
}

/// Helper struct to create a `RpcImage::Raw`.
pub struct RawImage; 
impl RawImage { pub fn new(key: &str) -> RpcImage { RpcImage::Raw(key.to_string()) } }

/// Helper struct to create a `RpcImage::Application`.
pub struct ApplicationImage; 
impl ApplicationImage { pub fn new(name: &str) -> RpcImage { RpcImage::Application(name.to_string()) } }

impl RpcImage {
    /// Auto-detects the image type from a string string.
    /// - Starts with `http`: External or Discord URL.
    /// - Starts with `attachments/`: Discord.
    /// - Alphanumeric (non-snowflake): Application Asset.
    /// - Else: Raw.
    pub fn from_string(source: &str) -> RpcImage {
        if source.starts_with("https://") || source.starts_with("http://") {
            if let Ok(url) = Url::parse(source) {
                if let Some(host) = url.host_str() {
                    if host == "cdn.discordapp.com" || host == "media.discordapp.net" {
                         let path = url.path().trim_start_matches('/');
                         return RpcImage::Discord(path.to_string());
                    }
                }
            }
            return RpcImage::External(source.to_string());
        }
        
        if source.starts_with("attachments/") || source.starts_with("external/") {
            return RpcImage::Discord(source.to_string());
        }

        let is_alphanumeric = source.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        let is_snowflake = source.len() >= 17 && source.len() <= 20 && source.chars().all(|c| c.is_ascii_digit());
        if is_alphanumeric && !is_snowflake {
            return RpcImage::Application(source.to_string());
        }

        RpcImage::Raw(source.to_string())
    }

    /// Resolves the image source into a string key usable by Discord.
    /// May involve HTTP requests (uploads/proxying).
    pub async fn resolve(&self, imageService: &ImageService) -> Option<String> {
        match self {
            RpcImage::Discord(key) => {
                if key.starts_with("mp:") { Some(key.clone()) } else { Some(format!("mp:{}", key)) }
            },
            RpcImage::External(url) => imageService.getExternalUrl(url).await,
            RpcImage::Local { path, name } => imageService.uploadImage(path, name).await,
            RpcImage::Raw(key) => Some(key.clone()),
            RpcImage::Application(name) => Some(format!("app_asset:{}", name)),
        }
    }

    pub fn getCacheKey(&self) -> String {
        match self {
            RpcImage::Discord(key) => format!("discord:{}", key),
            RpcImage::External(url) => format!("external:{}", url),
            RpcImage::Local { path, .. } => format!("local:{}", path),
            RpcImage::Raw(key) => format!("raw:{}", key),
            RpcImage::Application(name) => format!("app_asset:{}", name),
        }
    }
}