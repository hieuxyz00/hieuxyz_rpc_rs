use serde::Serialize;

#[derive(Serialize, Debug, Clone)]
pub struct ClientProperties {
    pub os: String,
    pub browser: String,
    pub device: String,
}

#[derive(Serialize, Debug)]
pub struct IdentifyPayload {
    pub token: String,
    pub capabilities: u32,
    pub large_threshold: u32, 
    pub properties: ClientProperties,
    pub compress: bool,
}

#[allow(non_snake_case)]
pub fn getIdentifyPayload(token: &str, properties: Option<ClientProperties>) -> IdentifyPayload {
    let defaultProperties = ClientProperties {
        os: "Windows".to_string(),
        browser: "Discord Client".to_string(),
        device: "hieuxyz©rpc".to_string(),
    };

    let finalProps = match properties {
        Some(p) => ClientProperties {
            os: if !p.os.is_empty() { p.os } else { defaultProperties.os },
            browser: if !p.browser.is_empty() { p.browser } else { defaultProperties.browser },
            device: if !p.device.is_empty() { p.device } else { defaultProperties.device },
        },
        None => defaultProperties,
    };

    IdentifyPayload {
        token: token.to_string(),
        capabilities: 65,
        large_threshold: 50,
        properties: finalProps,
        compress: true,
    }
}