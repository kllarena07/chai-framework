# Chai ☕️🌿

A framework for creating TUI SSH programs in Rust, powered by [ratatui](https://github.com/ratatui/ratatui) and [russh](https://github.com/Eugeny/russh).

**Note:** Chai uses [semantic versioning](https://www.geeksforgeeks.org/software-engineering/introduction-semantic-versioning/). Please be aware of this when using the framework

---

## Getting Started

### 1. Add dependencies

```sh
cargo add chai-framework tokio tracing-subscriber
```

### 2. Implement your TUI app

```rust
use chai_framework::ChaiApp;
use ratatui::prelude::*;

pub struct MyApp {
    quit: bool,
}

impl ChaiApp for MyApp {
    fn new() -> Self {
        Self { quit: false }
    }

    fn update(&mut self) {
        // update state periodically (called by the server)
    }

    fn draw(&mut self, f: &mut Frame) {
        let text = Paragraph::new("Hello from Chai!");
        f.render_widget(text, f.size());
    }

    fn handle_input(&mut self, data: &[u8]) {
        if data == b"q" {
            self.quit = true;
        }
    }

    fn should_quit(&self) -> bool {
        self.quit
    }
}
```

---

## Why Chai

The Chai framework makes it easy to host your ratatui apps on an SSH server.

First, encapsulate your TUI program within a stateful struct. Then, implement the `ChaiApp` trait for this struct to satisfy the required interface abstractions. After that, it's simple plug-and-play by providing your new struct to the `ChaiServer`.

---

## Running a Chai Server

```rust
mod app;
use app::MyApp; // your TUI app

use chai_framework::{ChaiServer, load_host_keys};
use russh::{MethodKind, MethodSet, server::Config};

#[tokio::main]
async fn main() {
    // Load a private key (for example: ~/.ssh/id_ed25519)
    let host_key = load_host_keys("/home/your_user/.ssh/id_ed25519")
        .expect("Failed to load host key");

    let mut methods = MethodSet::empty();
    methods.push(MethodKind::None);

    let config = Config {
        keys: vec![host_key],
        methods,
        ..Default::default()
    };

    let mut server = ChaiServer::<MyApp>::new(2222)
        .with_max_connections(32)      // optional (default: 100)
        .with_channel_buffer(1024);    // optional (default: 64)

    server
        .run(config)
        .await
        .expect("Failed running server");
}
```

---

## Logging

Chai uses `tracing`.

You can control logging with environment variables:

```sh
RUST_LOG=info cargo run
```

---

## Examples

For examples, see [here](https://github.com/kllarena07/chai/tree/main/examples). Simply run `cargo run --example [example name]`.

---

## Contributors

<a href="https://github.com/kllarena07/chai-framework/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=kllarena07/chai-framework&v=2026-02-26" />
</a>
