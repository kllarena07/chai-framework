# Chai ☕️🌿

A framework for creating TUI SSH programs in Rust, powered by [ratatui](https://github.com/ratatui/ratatui) and [russh](https://github.com/Eugeny/russh).

## Getting Started

1. Add the necessary crates:

```
cargo add chai-framework tokio tracing-subscriber
```

2. (Optional but recommended) Enable logging with environment filtering:

```rust
use tracing_subscriber::EnvFilter;

fn init_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();
}
```

3. Configure your main function, see under "Why Chai"

---

## Why Chai

The Chai framework makes it easy to host your ratatui apps on an SSH server.

First, encapsulate your TUI program within a stateful struct. Then, implement the `ChaiApp` trait for this struct to satisfy the required interface abstractions. After that, it's simple plug-and-play by providing your new struct to the `ChaiServer`.

```
mod app;
use app::MyApp; // your TUI program
use chai_framework::{ChaiApp, ChaiServer, load_system_host_keys};
use russh::server::Config;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    init_logging();

    // this loads a host key from ~/.ssh/id_ed25519
    let host_key = load_system_host_keys("id_ed25519");

    let config = Arc::new(Config {
        keys: vec![host_key],
        ..Default::default()
    });

    let server = ChaiServer::<MyApp>::builder()
        .port(2222)
        .max_connections(100)
        .channel_buffer_size(2048)
        .build();

    server.run(config).await.expect("Failed running server");
}
```

For examples, see [here](https://github.com/kllarena07/chai/tree/main/examples). Simply run `cargo run --example [example name]`.

---


## ChaiApp (v2.0)

In v2.0, `ChaiApp` was extended with a default `should_quit()` method, allowing applications to gracefully terminate sessions.

```rust
pub trait ChaiApp {
    fn on_input(&mut self, input: &str);
    fn draw(&mut self, frame: &mut ratatui::Frame);

    fn should_quit(&self) -> bool {
        false
    }
}
```

Example state struct with quit support:

```rust
pub struct MyApp {
    counter: u32,
    quit: bool,
}

impl ChaiApp for MyApp {
    fn on_input(&mut self, input: &str) {
        match input.trim() {
            "q" => self.quit = true,
            "inc" => self.counter += 1,
            _ => {}
        }
    }

    fn should_quit(&self) -> bool {
        self.quit
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        // draw UI
    }
}
```

---

## Server Improvements (v2.0)

ChaiServer now includes:

* Configurable maximum concurrent connections via `.max_connections(usize)`
* Configurable SSH channel buffer size via `.channel_buffer_size(usize)`
* Improved client lifecycle management
* Proper cleanup on session close and drop
* Improved input validation
* Enhanced error logging using `tracing`

`russh` has been updated to `0.57`, and `tracing-subscriber` now supports the `env-filter` feature for better log control.

---
## Contributors

<a href="https://github.com/kllarena07/chai-framework/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=kllarena07/chai-framework&v=2026-02-23" />
</a>
