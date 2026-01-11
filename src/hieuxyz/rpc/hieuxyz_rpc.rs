#![allow(non_snake_case)]
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use tokio::task::JoinHandle;
use futures_util::future::BoxFuture;
use url::Url;

use crate::hieuxyz::gateway::entities::types::*;
use crate::hieuxyz::utils::logger::logger;
use super::image_service::ImageService;
use super::rpc_image::RpcImage;

/// Helper enum to accept either a raw string or an RpcImage object for image setters.
pub enum ImageSource {
    Str(String),
    Obj(RpcImage),
}
impl From<&str> for ImageSource { fn from(s: &str) -> Self { ImageSource::Str(s.to_string()) } }
impl From<String> for ImageSource { fn from(s: String) -> Self { ImageSource::Str(s) } }
impl From<RpcImage> for ImageSource { fn from(o: RpcImage) -> Self { ImageSource::Obj(o) } }

pub type UpdateCallback = Arc<dyn Fn() -> BoxFuture<'static, ()> + Send + Sync>;

/// Main builder class for constructing Discord Rich Presence activities.
///
/// Methods in this struct are chainable.
/// You typically access this via `client.rpc.write().await`.
pub struct HieuxyzRPC {
    imageService: ImageService,
    onUpdate: UpdateCallback,
    activity: Activity,
    large_image_src: Option<RpcImage>,
    small_image_src: Option<RpcImage>,
    large_text: Option<String>,
    small_text: Option<String>,
    status: String,
    resolvedAssetsCache: Arc<Mutex<HashMap<String, String>>>,
    applicationAssetsCache: Arc<Mutex<HashMap<String, HashMap<String, String>>>>,
    renewalTask: Option<JoinHandle<()>>,
    MAX_CACHE_SIZE: usize,
}

impl HieuxyzRPC {
    pub fn new(imageService: ImageService, onUpdate: UpdateCallback) -> Self {
        let mut rpc = Self {
            imageService,
            onUpdate,
            activity: Activity {
                application_id: Some("1416676323459469363".to_string()),
                platform: Some("desktop".to_string()),
                name: "hieuxyzRPC".to_string(),
                r#type: 0,
                ..Default::default()
            },
            large_image_src: None,
            small_image_src: None,
            large_text: None,
            small_text: None,
            status: "online".to_string(),
            resolvedAssetsCache: Arc::new(Mutex::new(HashMap::new())),
            applicationAssetsCache: Arc::new(Mutex::new(HashMap::new())),
            renewalTask: None,
            MAX_CACHE_SIZE: 50,
        };
        rpc.startBackgroundRenewal();
        rpc
    }

    fn sanitize(&self, str: &str, len: usize) -> String { if str.len() > len { str[0..len].to_string() } else { str.to_string() } }

    /// Sets the name of the activity (First line).
    ///
    /// # Arguments
    /// * `name` - The name to display.
    pub fn setName(&mut self, name: &str) -> &mut Self { self.activity.name = self.sanitize(name, 128); self }

    /// Sets the details of the activity (Second line).
    ///
    /// # Arguments
    /// * `details` - Details to display.
    pub fn setDetails(&mut self, details: &str) -> &mut Self { self.activity.details = Some(self.sanitize(details, 128)); self }

    /// Sets the state of the activity (Third line).
    ///
    /// # Arguments
    /// * `state` - State to display.
    pub fn setState(&mut self, state: &str) -> &mut Self { self.activity.state = Some(self.sanitize(state, 128)); self }

    /// Sets the activity type.
    ///
    /// # Arguments
    /// * `t` - The type of activity (e.g. 0: Playing, 1: Streaming, 2: Listening, 3: Watching, 5: Competing).
    pub fn setType(&mut self, t: u8) -> &mut Self { self.activity.r#type = t; self }

    /// Sets the start and end timestamps. Pass `None` to ignore a field.
    ///
    /// # Arguments
    /// * `start` - Unix timestamp (milliseconds) for start time.
    /// * `end` - Unix timestamp (milliseconds) for end time.
    pub fn setTimestamps(&mut self, start: Option<u64>, end: Option<u64>) -> &mut Self { self.activity.timestamps = Some(ActivityTimestamps { start, end }); self }

    /// Sets the party information (current size and max size).
    ///
    /// # Arguments
    /// * `current` - Current number of players.
    /// * `max` - Maximum number of players.
    /// * `id` - (Optional) Custom Party ID. If `None`, defaults to "hieuxyz".
    pub fn setParty(&mut self, current: u32, max: u32, id: Option<&str>) -> &mut Self {
        let party_id = id.unwrap_or("hieuxyz").to_string();
        self.activity.party = Some(ActivityParty { id: Some(party_id), size: Some([current, max]) });
        self
    }

    /// Sets the large image and hover text.
    ///
    /// # Arguments
    /// * `source` - Image source (URL, asset key, Asset Name or RpcImage object).
    /// * `text` - Text displayed when hovering over image.
    pub fn setLargeImage<T: Into<ImageSource>>(&mut self, source: T, text: Option<&str>) -> &mut Self {
        let src = match source.into() {
            ImageSource::Str(s) => RpcImage::from_string(&s),
            ImageSource::Obj(o) => o,
        };
        self.large_image_src = Some(src);
        self.large_text = text.map(|t| self.sanitize(t, 128));
        self
    }

    /// Sets the small image and hover text.
    ///
    /// # Arguments
    /// * `source` - Image source (URL, asset key, Asset Name or RpcImage object).
    /// * `text` - Text displayed when hovering over image.
    pub fn setSmallImage<T: Into<ImageSource>>(&mut self, source: T, text: Option<&str>) -> &mut Self {
        let src = match source.into() {
            ImageSource::Str(s) => RpcImage::from_string(&s),
            ImageSource::Obj(o) => o,
        };
        self.small_image_src = Some(src);
        self.small_text = text.map(|t| self.sanitize(t, 128));
        self
    }

    /// Adds a single button to the activity.
    ///
    /// # Arguments
    /// * `label` - The text displayed on the button.
    /// * `url` - The URL opened when the button is clicked.
    pub fn addButton(&mut self, label: &str, url: &str) -> &mut Self {
        let final_label = self.sanitize(label, 32);
        if self.activity.buttons.is_none() {
            self.activity.buttons = Some(Vec::new());
        }
        if self.activity.metadata.is_none() {
            self.activity.metadata = Some(ActivityMetadata { button_urls: Vec::new() });
        }
        let buttons = self.activity.buttons.as_mut().unwrap();
        let metadata = self.activity.metadata.as_mut().unwrap();
        if buttons.len() >= 2 {
            logger::warn("Cannot add more than 2 buttons. Button ignored.");
            return self;
        }
        buttons.push(final_label);
        metadata.button_urls.push(url.to_string());
        self
    }

    /// Sets the buttons for the activity.
    /// Takes a vector of tuples `(Label, URL)`. Max 2 buttons.
    /// This will overwrite any existing buttons.
    pub fn setButtons(&mut self, buttons: Vec<(String, String)>) -> &mut Self {
        let valid_buttons: Vec<_> = buttons.into_iter().take(2).collect();
        let labels = valid_buttons.iter().map(|b| self.sanitize(&b.0, 32)).collect();
        let urls = valid_buttons.iter().map(|b| b.1.clone()).collect();
        self.activity.buttons = Some(labels);
        self.activity.metadata = Some(ActivityMetadata { button_urls: urls });
        self
    }

    /// Sets secrets for join/spectate/match functionality.
    pub fn setSecrets(&mut self, join: Option<String>, spectate: Option<String>, match_secret: Option<String>) -> &mut Self {
        self.activity.secrets = Some(ActivitySecrets { join, spectate, r#match: match_secret }); self
    }

    /// Sets the Sync ID (e.g. for Spotify integration).
    ///
    /// # Arguments
    /// * `syncId` - The synchronization ID.
    pub fn setSyncId(&mut self, syncId: String) -> &mut Self { self.activity.sync_id = Some(syncId); self }

    /// Sets activity flags.
    ///
    /// # Arguments
    /// * `flags` - A number representing the bitwise flags.
    ///
    /// # Example
    /// ```no_run
    /// rpc.setFlags(ActivityFlags::JOIN | ActivityFlags::INSTANCE);
    /// ```
    pub fn setFlags(&mut self, flags: u32) -> &mut Self { self.activity.flags = Some(flags); self }

    /// Sets a custom Application ID.
    ///
    /// # Arguments
    /// * `id` - Discord app ID (must be an 18 or 19 digit number string).
    ///
    /// # Panics
    /// Panics if the ID is not a valid snowflake string (17-20 digits).
    pub fn setApplicationId(&mut self, id: &str) -> &mut Self {
        let is_valid = id.len() >= 17 && id.len() <= 20 && id.chars().all(|c| c.is_ascii_digit());
        if !is_valid { panic!("The app ID must be a valid number string (17-20 digits)."); }
        self.activity.application_id = Some(id.to_string()); self
    }

    /// Sets the platform.
    /// Accepts `DiscordPlatform` enum or a string.
    ///
    /// # Example
    /// ```no_run
    /// rpc.setPlatform(DiscordPlatform::Desktop);
    /// // or
    /// rpc.setPlatform("android");
    /// ```
    pub fn setPlatform<T: Into<PlatformSource>>(&mut self, platform: T) -> &mut Self {
        let s = match platform.into() {
            PlatformSource::Enum(p) => p.as_str().to_string(),
            PlatformSource::Str(s) => s,
        };
        self.activity.platform = Some(s);
        self
    }

    /// Marks the activity as an instance (joinable).
    pub fn setInstance(&mut self, instance: bool) -> &mut Self { self.activity.instance = Some(instance); self }

    /// Sets the user status (e.g., "online", "dnd", "idle").
    pub fn setStatus(&mut self, status: &str) -> &mut Self { self.status = status.to_string(); self }

    fn _resolveAssetUrl(&self, assetKey: &str) -> String {
        if assetKey.starts_with("mp:") { return format!("https://media.discordapp.net/{}", &assetKey[3..]); }
        if assetKey.starts_with("spotify:") { return format!("https://i.scdn.co/image/{}", &assetKey[8..]); }
        if assetKey.starts_with("youtube:") { return format!("https://i.ytimg.com/vi/{}/hqdefault.jpg", &assetKey[8..]); }
        if assetKey.starts_with("twitch:") { return format!("https://static-cdn.jtvnw.net/previews-ttv/live_user_{}.png", &assetKey[7..]); }

        if let Some(app_id) = &self.activity.application_id {
            if !assetKey.starts_with("http") { return format!("https://cdn.discordapp.com/app-assets/{}/{}.png", app_id, assetKey); }
        }
        String::new()
    }

    fn getExpiryTime(assetKey: &str) -> Option<u64> {
        if !assetKey.starts_with("mp:attachments") { return None; }
        let url_part = &assetKey[3..];
        let url_str = format!("https://cdn.discordapp.com/{}", url_part);
        if let Ok(u) = Url::parse(&url_str) {
            if let Some(ex) = u.query_pairs().find(|(k, _)| k == "ex").map(|(_, v)| v) {
                if let Ok(ts) = u64::from_str_radix(&ex, 16) { return Some(ts * 1000); }
            }
        }
        None
    }

    async fn renewAssetIfNeeded(&self, cacheKey: &str, assetKey: &str) -> String {
        if let Some(expiry) = Self::getExpiryTime(assetKey) {
             let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
             if expiry < now + 3600000 {
                 let parts: Vec<&str> = assetKey.split("mp:attachments/").collect();
                 if parts.len() > 1 {
                     if let Some(new_id) = self.imageService.renewImage(parts[1]).await {
                         self.resolvedAssetsCache.lock().unwrap().insert(cacheKey.to_string(), new_id.clone());
                         return new_id;
                     }
                 }
                 logger::warn("Failed to renew asset, will use old one.");
             }
        }
        assetKey.to_string()
    }

    fn resolveImage<'a>(&'a self, image: Option<RpcImage>) -> BoxFuture<'a, Option<String>> {
        Box::pin(async move {
            let img = image?;
            let cacheKey = img.getCacheKey();

            if cacheKey.starts_with("app_asset:") {
                 self.ensureAppAssetsLoaded().await;
                 let name = cacheKey.strip_prefix("app_asset:")?;
                 let app_id = self.activity.application_id.as_ref()?;

                 let resolved = {
                     let cache = self.applicationAssetsCache.lock().unwrap();
                     cache.get(app_id).and_then(|map| map.get(name).cloned())
                 };

                 if let Some(id) = resolved { return Some(id); }
                 logger::warn(&format!("Asset with name \"{}\" not found for Application ID {}.", name, app_id));
                 return None;
            }

            let cached_val = {
                let mut cache = self.resolvedAssetsCache.lock().unwrap();
                if cache.len() >= self.MAX_CACHE_SIZE && !cache.contains_key(&cacheKey) {
                     let k = cache.keys().next().cloned();
                     if let Some(key) = k { cache.remove(&key); }
                }
                cache.get(&cacheKey).cloned()
            };

            if let Some(val) = cached_val {
                 return Some(self.renewAssetIfNeeded(&cacheKey, &val).await);
            }

            if let Some(resolved) = img.resolve(&self.imageService).await {
                 if resolved.starts_with("app_asset:") {
                     let recursive_img = RpcImage::Application(resolved["app_asset:".len()..].to_string());
                     return self.resolveImage(Some(recursive_img)).await;
                 }
                 self.resolvedAssetsCache.lock().unwrap().insert(cacheKey, resolved.clone());
                 return Some(resolved);
            }
            None
        })
    }

    async fn ensureAppAssetsLoaded(&self) {
        if let Some(app_id) = &self.activity.application_id {
            let needs_load = !self.applicationAssetsCache.lock().unwrap().contains_key(app_id);
            if needs_load {
                 logger::info(&format!("Fetching assets for Application ID: {}...", app_id));
                 let assets = self.imageService.fetchApplicationAssets(app_id).await;
                 let mut map = HashMap::new();
                 for a in assets { map.insert(a.name, a.id); }
                 let count = map.len();
                 self.applicationAssetsCache.lock().unwrap().insert(app_id.clone(), map);
                 logger::info(&format!("Loaded {} assets for Application ID: {}.", count, app_id));
            }
        }
    }

    fn startBackgroundRenewal(&mut self) {
        let cache = self.resolvedAssetsCache.clone();
        let service = self.imageService.clone();

        self.renewalTask = Some(tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(600000));
            loop {
                interval.tick().await;
                let mut entries = Vec::new();
                {
                    let lock = cache.lock().unwrap();
                    for (k, v) in lock.iter() { entries.push((k.clone(), v.clone())); }
                }

                for (key, assetKey) in entries {
                    if let Some(expiry) = Self::getExpiryTime(&assetKey) {
                        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
                         if expiry < now + 3600000 {
                             let parts: Vec<&str> = assetKey.split("mp:attachments/").collect();
                             if parts.len() > 1 {
                                 if let Some(new_id) = service.renewImage(parts[1]).await {
                                     cache.lock().unwrap().insert(key, new_id);
                                 }
                             }
                         }
                    }
                }
            }
        }));
    }

    /// Constructs the activity object, resolving any pending images.
    /// This is used internally by `sendAllActivities`.
    pub async fn buildActivity(&self) -> Option<Activity> {
        let mut final_activity = self.activity.clone();
        if final_activity.details.is_none()
           && final_activity.state.is_none()
           && self.large_image_src.is_none()
           && self.small_image_src.is_none()
           && (final_activity.name.is_empty() || final_activity.name == "hieuxyzRPC") {
             return None;
        }

        let large = self.resolveImage(self.large_image_src.clone()).await;
        let small = self.resolveImage(self.small_image_src.clone()).await;

        if large.is_some() || small.is_some() {
            final_activity.assets = Some(ActivityAssets {
                large_image: large, large_text: self.large_text.clone(),
                small_image: small, small_text: self.small_text.clone(),
            });
        } else {
            final_activity.assets = None;
        }

        if final_activity.name.is_empty() { final_activity.name = "hieuxyzRPC".to_string(); }

        Some(final_activity)
    }

    /// Builds and sends the presence update to Discord.
    /// This triggers the client's update mechanism.
    pub async fn build(&self) { (self.onUpdate)().await; }

    /// Alias for `build()`.
    pub async fn updateRPC(&self) { self.build().await; }

    /// Clears the current RPC settings (resets to default).
    pub fn clear(&mut self) {
        self.activity = Activity::default();
        self.activity.application_id = Some("1416676323459469363".to_string());
        self.activity.platform = Some("desktop".to_string());
        self.large_image_src = None;
        self.small_image_src = None;
        self.large_text = None;
        self.small_text = None;
        logger::info("RPC instance cleared.");
        let on_update = self.onUpdate.clone();
        tokio::spawn(async move {
            (on_update)().await;
        });
    }

    /// Clears the asset cache.
    pub fn clearCache(&self) {
        self.resolvedAssetsCache.lock().unwrap().clear();
        self.applicationAssetsCache.lock().unwrap().clear();
        logger::info("RPC Asset cache has been cleared.");
    }

    /// Destroys the RPC instance, stopping background tasks and clearing state.
    pub fn destroy(&mut self) {
        self.stopBackgroundRenewal();
        self.clearCache();
        self.activity = Activity::default();
    }

    pub fn stopBackgroundRenewal(&mut self) {
        if let Some(t) = &self.renewalTask { t.abort(); self.renewalTask = None; }
    }

    pub fn get_currentStatus(&self) -> String { self.status.clone() }
}