use crate::chai::ChaiApp;

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;

use ratatui::{Terminal, TerminalOptions, Viewport};
use russh::keys::ssh_key::PublicKey;
use russh::server::*;
use russh::{Channel, ChannelId, Pty};
use tokio::sync::Mutex;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tracing::{Level, event};

const ENTER_ALT_SCREEN: &[u8] = b"\x1b[?1049h";
const EXIT_ALT_SCREEN: &[u8] = b"\x1b[?1049l";
const HIDE_CURSOR: &[u8] = b"\x1b[?25l";
const SHOW_CURSOR: &[u8] = b"\x1b[?25h";

type SshTerminal = Terminal<CrosstermBackend<TerminalHandle>>;

struct TerminalHandle {
    sender: UnboundedSender<Vec<u8>>,
    // The sink collects the data which is finally sent to sender.
    sink: Vec<u8>,
}

impl TerminalHandle {
    async fn start(handle: Handle, channel_id: ChannelId) -> Self {
        let (sender, mut receiver) = unbounded_channel::<Vec<u8>>();
        tokio::spawn(async move {
            while let Some(data) = receiver.recv().await {
                let result = handle.data(channel_id, data.into()).await;
                if result.is_err() {
                    eprintln!("Failed to send data: {result:?}");
                }
            }
        });
        Self {
            sender,
            sink: Vec::new(),
        }
    }
}

impl std::io::Write for TerminalHandle {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.sink.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let result = self.sender.send(self.sink.clone());
        if result.is_err() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                result.unwrap_err(),
            ));
        }

        self.sink.clear();
        Ok(())
    }
}

#[derive(Clone)]
pub struct ChaiServer<T: ChaiApp + Send + 'static> {
    clients: Arc<Mutex<HashMap<usize, (SshTerminal, T)>>>,
    port: u16,
    id: usize,
}

impl<T: ChaiApp + Send + 'static> ChaiServer<T> {
    pub fn new(port: u16) -> Self {
        Self {
            clients: Arc::new(Mutex::new(HashMap::new())),
            port,
            id: 0,
        }
    }

    pub async fn run(&mut self, config: Config) -> Result<(), anyhow::Error> {
        let clients = self.clients.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

                for (_, (terminal, app)) in clients.lock().await.iter_mut() {
                    terminal
                        .draw(|f| {
                            app.update();
                            app.draw(f);
                        })
                        .unwrap();
                }
            }
        });

        event!(Level::INFO, "starting chai server on 0.0.0.0:{}", self.port);
        self.run_on_address(Arc::new(config), ("0.0.0.0", self.port))
            .await?;
        Ok(())
    }
}

impl<T: ChaiApp + Send + 'static> Server for ChaiServer<T> {
    type Handler = Self;
    fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> Self {
        let s = self.clone();
        self.id += 1;
        s
    }
}

impl<T: ChaiApp + Send + 'static> Handler for ChaiServer<T> {
    type Error = anyhow::Error;

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        session: &mut Session,
    ) -> Result<bool, Self::Error> {
        let terminal_handle = TerminalHandle::start(session.handle(), channel.id()).await;

        let backend = CrosstermBackend::new(terminal_handle);

        // the correct viewport area will be set when the client request a pty
        let options = TerminalOptions {
            viewport: Viewport::Fixed(Rect::default()),
        };

        let terminal = Terminal::with_options(backend, options)?;
        let app = T::new();

        let mut clients = self.clients.lock().await;
        clients.insert(self.id, (terminal, app));

        Ok(true)
    }

    async fn auth_publickey(&mut self, _: &str, _: &PublicKey) -> Result<Auth, Self::Error> {
        Ok(Auth::Accept)
    }

    async fn auth_none(&mut self, _: &str) -> Result<Auth, Self::Error> {
        Ok(Auth::Accept)
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        match data {
            // Pressing 'q' closes the connection.
            b"q" => {
                if let Err(e) = session.data(channel, EXIT_ALT_SCREEN.into()) {
                    eprintln!("Failed to exit alternate screen: {:?}", e);
                }

                if let Err(e) = session.data(channel, SHOW_CURSOR.into()) {
                    eprintln!("Failed to show cursor: {:?}", e);
                }

                self.clients.lock().await.remove(&self.id);
                session.close(channel)?;
            }
            _ => {
                let mut clients = self.clients.lock().await;
                let (_, app) = clients.get_mut(&self.id).unwrap();
                app.handle_input(data);
            }
        }

        Ok(())
    }

    async fn window_change_request(
        &mut self,
        _: ChannelId,
        col_width: u32,
        row_height: u32,
        _: u32,
        _: u32,
        _: &mut Session,
    ) -> Result<(), Self::Error> {
        let rect = Rect {
            x: 0,
            y: 0,
            width: col_width as u16,
            height: row_height as u16,
        };

        let mut clients = self.clients.lock().await;
        let (terminal, _) = clients.get_mut(&self.id).unwrap();
        terminal.resize(rect)?;

        Ok(())
    }

    async fn pty_request(
        &mut self,
        channel: ChannelId,
        _: &str,
        col_width: u32,
        row_height: u32,
        _: u32,
        _: u32,
        _: &[(Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        let rect = Rect {
            x: 0,
            y: 0,
            width: col_width as u16,
            height: row_height as u16,
        };

        let mut clients = self.clients.lock().await;
        let (terminal, _) = clients.get_mut(&self.id).unwrap();
        terminal.resize(rect)?;

        session.channel_success(channel)?;

        if let Err(e) = session.data(channel, ENTER_ALT_SCREEN.into()) {
            eprintln!("Failed to enter alternate screen: {:?}", e);
        }

        if let Err(e) = session.data(channel, HIDE_CURSOR.into()) {
            eprintln!("Failed to hide cursor: {:?}", e);
        }

        Ok(())
    }

    async fn channel_close(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        let reset_sequence = [EXIT_ALT_SCREEN, SHOW_CURSOR].concat();
        let _ = session.data(channel, reset_sequence.into());

        self.clients.lock().await.remove(&self.id);
        Ok(())
    }
}

impl<T: ChaiApp + Send + 'static> Drop for ChaiServer<T> {
    fn drop(&mut self) {
        let id = self.id;
        let clients = self.clients.clone();
        tokio::spawn(async move {
            let mut clients = clients.lock().await;
            clients.remove(&id);
        });
    }
}

pub fn load_host_keys(key_path: Option<&str>) -> Result<russh::keys::PrivateKey, anyhow::Error> {
    let key_path = std::env::var("CHAI_HOST_KEY_PATH")
        .ok()
        .or(key_path.map(|s| s.to_string()))
        .unwrap_or_else(|| "./authorized_keys/ed_25519".to_string());
    let key_path = Path::new(&key_path);

    if !key_path.exists() {
        return Err(anyhow::anyhow!(
            "Host key not found at {}. Please generate host keys first.",
            key_path.display()
        ));
    }

    let key = russh::keys::PrivateKey::read_openssh_file(&key_path)
        .map_err(|e| anyhow::anyhow!("Failed to read host key: {}", e))?;

    Ok(key)
}
