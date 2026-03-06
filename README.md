# hieuxyz_rpc

> A powerful Discord Rich Presence library for Rust.
> **Note:** This project is a Rust port of the TypeScript library [@hieuxyz/rpc](https://github.com/hieuxyz00/hieuxyz_rpc).

[![Crates.io](https://img.shields.io/crates/v/hieuxyz_rpc.svg)](https://crates.io/crates/hieuxyz_rpc)
[![License](https://img.shields.io/crates/l/hieuxyz_rpc.svg)](LICENSE)

`hieuxyz_rpc` allows you to control the RPC status of a Discord **User Account** directly from Rust. It supports advanced features like multi-RPC, client spoofing, custom assets, etc...

> [!WARNING]
> **I do not take responsibility for any blocked Discord accounts resulting from the use of this library.**

> [!CAUTION]
> **Using this on a User Account is against the [Discord Terms of Service](https://discord.com/terms) (Self-botting) and may lead to account termination.**
> **By using this library, you accept the risk involved in exposing your Discord Token.**

## Installation

Add the following to your `Cargo.toml`:

```toml
[dependencies]
hieuxyz_rpc = "0.0.3"
tokio = { version = "1", features = ["full"] }
```

## Usage

### Basic Example

```rust
use hieuxyz_rpc::{Client, ClientOptions, logger};
use std::env;

#[tokio::main]
async fn main() {
    // Init the client
    let token = env::var("DISCORD_USER_TOKEN").expect("Token not set");
    let client = Client::new(ClientOptions {
        token,
        alwaysReconnect: Some(true),
        ..Default::default()
    });

    // Connect to Gateway
    client.run().await;

    // Update Rich Presence
    {
        // Acquire write lock to update status
        let mut rpc = client.rpc.write().await;
        
        rpc.setName("Visual Studio Code")
           .setDetails("Writing Rust code")
           .setState("Workspace: hieuxyz_rs")
           .setPlatform("desktop")
           .setStatus("dnd") // online, idle, dnd, invisible
           .setType(0) // 0: Playing
           .setLargeImage("https://rustacean.net/assets/rustacean-flat-happy.png", Some("Rust"))
           .setButtons(vec![
               ("View Repo".to_string(), "https://github.com/hieuxyz00/hieuxyz_rpc_rs".to_string())
           ]);
    }
    
    // Send the update to Discord
    {
        let rpc = client.rpc.read().await;
        rpc.build().await;
    }

    logger::info("RPC updated!");

    // Keep the process alive
    tokio::signal::ctrl_c().await.unwrap();
    client.close(true).await;
}
```

## Advanced Usage

### Client Spoofing (Mobile/Console Status)

You can make it appear as though you are using Discord from a different device (e.g., smartphone, Xbox) by providing `properties` in `ClientOptions`.

```rust
use hieuxyz_rpc::{Client, ClientOptions, ClientProperties};

let client = Client::new(ClientOptions {
    token: "YOUR_TOKEN".to_string(),
    properties: Some(ClientProperties {
        os: "Android".to_string(),
        browser: "Discord Android".to_string(),
        device: "Android16".to_string(),
    }),
    ..Default::default()
});
```

### Multi-RPC

You can display multiple statuses at once.

```rust
// Update default RPC
{
    let mut rpc = client.rpc.write().await;
    rpc.setName("Coding").setType(0);
}

// Create a second RPC instance
let music_rpc = client.createRPC().await;

{
    let mut rpc = music_rpc.write().await;
    rpc.setName("Spotify")
       .setDetails("Listening to generic lo-fi")
       .setType(2) // Listening
       .setApplicationId("12345678901234567"); // Must be different from the default App ID
}

// Send all activities
client.sendAllActivities().await;
```

## API Reference

### Struct Client

- `Client::new(options: ClientOptions)` -> Arc<Self>: Create a new instance.
  - `options.token`: Discord User Token.
  - `options.alwaysReconnect`: Auto reconnect on close (default: false).
  - `options.properties`: Spoof client properties (OS, browser).
  - `options.apiBaseUrl`: Custom image proxy URL.
  
- `client.run()` -> DiscordUser: Connect to Discord Gateway. Return user information when ready.
- `client.rpc`: Access the default HieuxyzRPC (use `.read().await` or `.write().await`).
- `client.createRPC()` -> Arc<RwLock<HieuxyzRPC>>: Create another RPC instance.
- `client.close(force: bool)`: Close the connection.

### Struct HieuxyzRPC

The Builder is used to set the Presence state. Note that write permissions (`write().await`) are required to call the set functions.

- `.setName(name: &str)`: Set name activity (line 1).
- `.setDetails(details: &str)`: Set details (line 2).
- `.setState(state: &str)`: Set state (line 3).
- `.setType(t: u8)`: Specify the activity type (0: Playing, 1: Streaming, 2: Listening, 3: Watching, 5: Competing).
- `.setStatus(status: &str)`: Set user status (`online`, `idle`, `dnd`, `invisible`, `offline`).
- `.setTimestamps(start: Option<u64>, end: Option<u64>)`: Set a time (Unix timestamp ms).
- `.setParty(current: u32, max: u32, id: Option<&str>)`: Set group information.
- `.setLargeImage(source, text)`: Set the large image. Source can be URL, asset key, or `RpcImage`.
- `.setSmallImage(source, text)`: Set the small image.
- `.setButtons(buttons: Vec<(String, String)>)`: Set buttons (Label, URL). Max 2.
- `.addButton(label: &str, url: &str)`: Add a single button.
- `.setApplicationId(id: &str)`: Set a custom App ID (Snowflake).
- `.setPlatform(platform: Into<PlatformSource>)`: Set platform (e.g., `DiscordPlatform::Desktop` or string "android").
- `.setFlags(flags: u32)`: Set activity flags (INSTANCE, JOIN, SPECTATE, etc.).
- `.setSyncId(id: String)`: Set Sync ID (e.g. for Spotify).
- `.setSecrets(join, spectate, match)`: Set game secrets.
- `.setInstance(bool)`: Mark as a game instance.
- `.build()` / `.updateRPC()`: Send the payload to Discord.
- `.clear()`: Clear current RPC state.
- `.clearCache()`: Clear resolved asset cache.

### Types of images (RpcImage)

- `RpcImage::from_string(url)`: Automatically detect image type from string.
- `DiscordImage::new(key)`: Use the assets available on Discord (e.g., mp:attachments/...).
- `ExternalImage::new(url)`: Use an external image URL (which will be proxyed).
- `LocalImage::new(path, name)`: Upload photos from your computer.
- `ApplicationImage::new(name)`: Use the Asset Name from the Discord Developer Portal.

## Author

- Developed by hieuxyz.
- GitHub: @hieuxyz00

## License

This project is licensed under the ISC License.
