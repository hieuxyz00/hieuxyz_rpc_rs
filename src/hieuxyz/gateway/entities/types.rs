use serde::{Deserialize, Serialize};
use super::opcode::OpCode;
use serde_json::Value;

pub struct ActivityFlags;

impl ActivityFlags {
    pub const INSTANCE: u32 = 1 << 0;
    pub const JOIN: u32 = 1 << 1;
    pub const SPECTATE: u32 = 1 << 2;
    pub const JOIN_REQUEST: u32 = 1 << 3;
    pub const SYNC: u32 = 1 << 4;
    pub const PLAY: u32 = 1 << 5;
    pub const PARTY_PRIVACY_FRIENDS: u32 = 1 << 6;
    pub const PARTY_PRIVACY_VOICE_CHANNEL: u32 = 1 << 7;
    pub const EMBEDDED: u32 = 1 << 8;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscordPlatform {
    Desktop,
    Android,
    Ios,
    Samsung,
    Xbox,
    Ps4,
    Ps5,
    Embedded,
}

impl DiscordPlatform {
    pub fn as_str(&self) -> &'static str {
        match self {
            DiscordPlatform::Desktop => "desktop",
            DiscordPlatform::Android => "android",
            DiscordPlatform::Ios => "ios",
            DiscordPlatform::Samsung => "samsung",
            DiscordPlatform::Xbox => "xbox",
            DiscordPlatform::Ps4 => "ps4",
            DiscordPlatform::Ps5 => "ps5",
            DiscordPlatform::Embedded => "embedded",
        }
    }
}

pub enum PlatformSource {
    Enum(DiscordPlatform),
    Str(String),
}
impl From<DiscordPlatform> for PlatformSource { fn from(p: DiscordPlatform) -> Self { PlatformSource::Enum(p) } }
impl From<&str> for PlatformSource { fn from(s: &str) -> Self { PlatformSource::Str(s.to_string()) } }
impl From<String> for PlatformSource { fn from(s: String) -> Self { PlatformSource::Str(s) } }

pub struct UserFlags;
impl UserFlags {
    pub const STAFF: u64 = 1 << 0;
    pub const PARTNER: u64 = 1 << 1;
    pub const HYPESQUAD: u64 = 1 << 2;
    pub const BUG_HUNTER_LEVEL_1: u64 = 1 << 3;
    pub const HYPESQUAD_ONLINE_HOUSE_1: u64 = 1 << 6;
    pub const HYPESQUAD_ONLINE_HOUSE_2: u64 = 1 << 7;
    pub const HYPESQUAD_ONLINE_HOUSE_3: u64 = 1 << 8;
    pub const PREMIUM_EARLY_SUPPORTER: u64 = 1 << 9;
    pub const TEAM_PSEUDO_USER: u64 = 1 << 10;
    pub const BUG_HUNTER_LEVEL_2: u64 = 1 << 14;
    pub const VERIFIED_BOT: u64 = 1 << 16;
    pub const VERIFIED_DEVELOPER: u64 = 1 << 17;
    pub const CERTIFIED_MODERATOR: u64 = 1 << 18;
    pub const BOT_HTTP_INTERACTIONS: u64 = 1 << 19;
    pub const ACTIVE_DEVELOPER: u64 = 1 << 22;
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GatewayPayload {
    pub op: OpCode,
    pub d: Option<Value>,
    pub s: Option<u64>,
    pub t: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DiscordUser {
    pub id: String,
    pub username: String,
    pub discriminator: String,
    pub global_name: Option<String>,
    pub avatar: Option<String>,
    pub bot: Option<bool>,
    pub flags: Option<u64>,
    pub premium_type: Option<u64>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub public_flags: Option<u64>,
    pub banner: Option<String>,
    pub accent_color: Option<u64>,
    pub banner_color: Option<String>,
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Activity {
    pub name: String,
    pub r#type: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub application_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flags: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub party: Option<ActivityParty>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamps: Option<ActivityTimestamps>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assets: Option<ActivityAssets>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secrets: Option<ActivitySecrets>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buttons: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ActivityMetadata>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ActivityParty {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<[u32; 2]>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ActivityTimestamps {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ActivityAssets {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub large_image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub large_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub small_image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub small_text: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ActivitySecrets {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub join: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spectate: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#match: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ActivityMetadata {
    pub button_urls: Vec<String>,
}

#[derive(Serialize, Debug, Clone)]
pub struct PresenceUpdatePayload {
    pub since: u64,
    pub activities: Vec<Activity>,
    pub status: String,
    pub afk: bool,
}