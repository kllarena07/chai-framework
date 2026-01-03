# Chai ☕️🌿

A framework for creating TUI SSH programs in Rust, powered by [ratatui](https://github.com/ratatui/ratatui) and [russh](https://github.com/Eugeny/russh).

## Getting Started
1. Add the necessary crates:
```
cargo add chai-framework tokio
```
2. Configure your main function, see below

## Why Chai
The Chai framework makes it easy to host your ratatui apps on an SSH server.

First, encapsulate your TUI program within a stateful struct. Then, implement the `ChaiApp` trait for this struct to satisfy the required interface abstractions. After that, it's simple plug-and-play by providing your new struct to the `ChaiServer`.
```
mod app;
use app::MyApp; // your TUI program
use chai_framework::{ChaiApp, ChaiServer, load_host_keys};

#[tokio::main]
async fn main() {
    let host_key = load_system_host_keys("id_ed25519");
    let config = Config {
        // server config here
        keys: vec![host_key],
    };

    let mut server = ChaiServer::<MyApp>::new(2222);
    server.run(config).await.expect("Failed running server");
}
```

For examples, see [here](https://github.com/kllarena07/chai/tree/main/examples).

_Made with ❤️ by [krayondev](https://x.com/krayondev)_
