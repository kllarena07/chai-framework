impl<T: ChaiApp + Send + 'static> ChaiServer<T> {
    /// Helper to get a mutable reference to a client, logging a warning if not found.
    fn get_client_mut<'a>(
        &self,
        clients: &'a mut HashMap<usize, (SshTerminal, T)>,
        context: &str,
        id: usize,
    ) -> Option<(&'a mut SshTerminal, &'a mut T)> {
        match clients.get_mut(&id) {
            Some((terminal, app)) => Some((terminal, app)),
            None => {
                tracing::warn!("No client found for id {} in {}", id, context);
                None
            }
        }
    }

    /// Helper to draw the app, logging error if it fails.
    fn try_draw(&self, terminal: &mut SshTerminal, app: &mut T) {
        if let Err(e) = terminal.draw(|f| app.draw(f)) {
            tracing::error!(
                "Terminal draw error for user {} (id: {}): {:?}",
                self.username,
                self.id,
                e
            );
        }
    }
}
use crate::chai::ChaiApp;

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use ratatui::layout::Rect;

use ratatui::{Terminal, TerminalOptions, Viewport};
use russh::keys::ssh_key::PublicKey;
use russh::server::*;
use russh::{Channel, ChannelId, Pty};
use tokio::sync::Mutex;
use tokio::sync::mpsc::{Sender, channel, error::TrySendError};

const ENTER_ALT_SCREEN: &[u8] = b"\x1b[?1049h";
const EXIT_ALT_SCREEN: &[u8] = b"\x1b[?1049l";
const HIDE_CURSOR: &[u8] = b"\x1b[?25l";
const SHOW_CURSOR: &[u8] = b"\x1b[?25h";
const CLEAR_ALL: &[u8] = b"\x1b[2J";

use ratatui::backend::CrosstermBackend;

type SshTerminal = Terminal<CrosstermBackend<TerminalHandle>>;

struct TerminalHandle {
    sender: Sender<Vec<u8>>,
    // The sink collects the data which is finally sent to sender.
    sink: Vec<u8>,
}

impl TerminalHandle {
    async fn start(
        handle: Handle,
        channel_id: ChannelId,
        username: String,
        id: usize,
        buffer: usize,
    ) -> Self {
        let (sender, mut receiver) = channel::<Vec<u8>>(buffer);
        let username_clone = username.clone();
        let id_clone = id;
        tokio::spawn(async move {
            while let Some(data) = receiver.recv().await {
                let result = handle.data(channel_id, data.into()).await;
                if result.is_err() {
                    tracing::debug!(
                        "channel closed for user {} (id: {}), stopping data forwarding",
                        username_clone,
                        id_clone
                    );
                    break;
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
        match self.sender.try_send(self.sink.clone()) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                // Consumer is slow; drop this frame rather than block the async runtime.
                tracing::debug!("terminal output buffer full, dropping frame");
            }
            Err(TrySendError::Closed(_)) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "terminal channel closed",
                ));
            }
        }

        self.sink.clear();
        Ok(())
    }
}

const DEFAULT_MAX_CONNECTIONS: usize = 100;
const DEFAULT_CHANNEL_BUFFER: usize = 64;

pub struct ChaiServer<T: ChaiApp + Send + 'static> {
    clients: Arc<Mutex<HashMap<usize, (SshTerminal, T)>>>,
    port: u16,
    id: usize,
    username: String,
    max_connections: usize,
    channel_buffer: usize,
    cols: u32,
    rows: u32,
}

impl<T: ChaiApp + Send + 'static> Clone for ChaiServer<T> {
    fn clone(&self) -> Self {
        Self {
            clients: self.clients.clone(),
            port: self.port,
            id: self.id,
            username: self.username.clone(),
            max_connections: self.max_connections,
            channel_buffer: self.channel_buffer,
            cols: 80,
            rows: 24,
        }
    }
}

impl<T: ChaiApp + Send + 'static> ChaiServer<T> {
    pub fn new(port: u16) -> Self {
        Self {
            clients: Arc::new(Mutex::new(HashMap::new())),
            port,
            id: 0,
            username: String::new(),
            max_connections: DEFAULT_MAX_CONNECTIONS,
            channel_buffer: DEFAULT_CHANNEL_BUFFER,
            cols: 80,
            rows: 24,
        }
    }

    /// Set the maximum number of concurrent SSH connections. Default: 100.
    pub fn with_max_connections(mut self, max: usize) -> Self {
        self.max_connections = max;
        self
    }

    /// Set the per-connection terminal output channel buffer size.
    /// Frames are dropped (not buffered) when the buffer is full. Default: 64.
    pub fn with_channel_buffer(mut self, size: usize) -> Self {
        self.channel_buffer = size;
        self
    }

    pub async fn run(&mut self, config: Config) -> Result<(), anyhow::Error> {
        let subscriber = tracing_subscriber::fmt()
            .compact()
            .with_file(true)
            .with_line_number(true)
            .with_target(true)
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "info".into()),
            )
            .finish();

        // Silently ignore the error if a global subscriber is already set.
        let _ = tracing::subscriber::set_global_default(subscriber);

        let clients = self.clients.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

                for (_, (terminal, app)) in clients.lock().await.iter_mut() {
                    if let Err(e) = terminal.draw(|f| {
                        app.update();
                        app.draw(f);
                    }) {
                        tracing::error!(
                            "Background terminal draw error in periodic updater: {:?}",
                            e
                        );
                    }
                }
            }
        });

        tracing::info!("starting server on 0.0.0.0:{}", self.port);
        self.run_on_address(Arc::new(config), ("0.0.0.0", self.port))
            .await?;
        Ok(())
    }
}

impl<T: ChaiApp + Send + 'static> ChaiServer<T> {
    fn send_data_or_log(
        &mut self,
        session: &mut Session,
        channel: ChannelId,
        data: &[u8],
        description: &str,
    ) {
        if let Err(e) = session.data(channel, data.into()) {
            tracing::error!(
                "failed to {} for user {} (id: {}): {:?}",
                description,
                self.username,
                self.id,
                e
            );
        }
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
        _channel: Channel<Msg>,
        _session: &mut Session,
    ) -> Result<bool, Self::Error> {
        let clients = self.clients.lock().await;
        if clients.len() >= self.max_connections {
            tracing::warn!(
                "max connections ({}) reached, rejecting session for {}",
                self.max_connections,
                self.username
            );
            return Ok(false);
        }

        tracing::info!("{} (id: {}) opened a channel", self.username, self.id);
        Ok(true)
    }

    async fn auth_publickey(&mut self, user: &str, _: &PublicKey) -> Result<Auth, Self::Error> {
        self.username = user.to_string();
        Ok(Auth::Accept)
    }

    async fn auth_none(&mut self, user: &str) -> Result<Auth, Self::Error> {
        self.username = user.to_string();
        Ok(Auth::Accept)
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        // Input validation: Only allow printable ASCII, control chars, and escape sequences
        if !data
            .iter()
            .all(|&b| b == b'\n' || b == b'\r' || b == 0x1b || (0x20..=0x7e).contains(&b))
        {
            tracing::warn!(
                "Received invalid input data from user {} (id: {})",
                self.username,
                self.id
            );
            return Ok(());
        }

        let should_quit = {
            let mut clients = self.clients.lock().await;
            if let Some((terminal, app)) =
                self.get_client_mut(&mut clients, "data handler", self.id)
            {
                app.handle_input(data);
                let quit = app.should_quit();
                if !quit {
                    self.try_draw(terminal, app);
                }
                quit
            } else {
                false
            }
        };

        if should_quit {
            self.send_data_or_log(session, channel, EXIT_ALT_SCREEN, "exit alternate screen");
            self.send_data_or_log(session, channel, SHOW_CURSOR, "show cursor");
            session.close(channel)?;
        }

        Ok(())
    }

    async fn window_change_request(
        &mut self,
        channel: ChannelId,
        col_width: u32,
        row_height: u32,
        _: u32,
        _: u32,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        let rect = Rect {
            x: 0,
            y: 0,
            width: col_width as u16,
            height: row_height as u16,
        };

        let mut clients = self.clients.lock().await;
        if let Some((_, mut app)) = clients.remove(&self.id) {
            // Recreate the terminal at the new size instead of calling
            // terminal.resize()
            let terminal_handle = TerminalHandle::start(
                session.handle(),
                channel,
                self.username.clone(),
                self.id,
                self.channel_buffer,
            )
            .await;

            let backend = CrosstermBackend::new(terminal_handle);
            let options = TerminalOptions {
                viewport: Viewport::Fixed(rect),
            };
            let mut terminal = Terminal::with_options(backend, options)?;

            // Queue a full screen clear into the sink (no flush yet) so that
            // it ships in the same message as the first draw frame.
            {
                use std::io::Write;
                terminal.backend_mut().write_all(CLEAR_ALL)?;
            }

            self.try_draw(&mut terminal, &mut app);
            clients.insert(self.id, (terminal, app));
        }

        Ok(())
    }

    async fn pty_request(
        &mut self,
        _channel: ChannelId,
        _: &str,
        col_width: u32,
        row_height: u32,
        _: u32,
        _: u32,
        _: &[(Pty, u32)],
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.cols = col_width;
        self.rows = row_height;
        Ok(())
    }

    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        let handle = session.handle();

        let terminal_handle = TerminalHandle::start(
            handle,
            channel,
            self.username.clone(),
            self.id,
            self.channel_buffer,
        )
        .await;

        let rect = Rect {
            x: 0,
            y: 0,
            width: self.cols as u16,
            height: self.rows as u16,
        };

        let backend = CrosstermBackend::new(terminal_handle);
        let options = TerminalOptions {
            viewport: Viewport::Fixed(rect),
        };
        let mut terminal = Terminal::with_options(backend, options)?;
        let mut app = T::new();

        // Send escape sequences through the terminal backend so they share
        // the same ordered mpsc channel as draw output.
        {
            use std::io::Write;
            let backend = terminal.backend_mut();
            backend.write_all(ENTER_ALT_SCREEN)?;
            backend.write_all(HIDE_CURSOR)?;
            backend.flush()?;
        }

        self.try_draw(&mut terminal, &mut app);
        self.clients.lock().await.insert(self.id, (terminal, app));

        session.channel_success(channel)?;

        Ok(())
    }

    async fn channel_close(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        tracing::info!("{} (id: {}) closed a channel", self.username, self.id);
        let reset_sequence = [EXIT_ALT_SCREEN, SHOW_CURSOR].concat();
        let _ = session.data(channel, reset_sequence.into());

        let mut clients = self.clients.lock().await;
        if clients.remove(&self.id).is_none() {
            tracing::warn!("No client found for id {} in channel_close", self.id);
        }
        Ok(())
    }
}

impl<T: ChaiApp + Send + 'static> Drop for ChaiServer<T> {
    fn drop(&mut self) {
        let id = self.id;
        let clients = self.clients.clone();
        if let Ok(mut guard) = clients.try_lock() {
            guard.remove(&id);
        } else {
            // If we can't lock, log and skip
            tracing::warn!("Could not lock clients for cleanup in Drop for id {}", id);
        }
    }
}

pub fn load_system_host_keys(key_name: &str) -> Result<russh::keys::PrivateKey, anyhow::Error> {
    let key_path = Path::new("/.ssh").join(key_name);

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

pub fn load_host_keys(path: &str) -> Result<russh::keys::PrivateKey, anyhow::Error> {
    let key_path = Path::new(path);

    if !key_path.exists() {
        return Err(anyhow::anyhow!(
            "Host key not found at {}. Please generate host keys first.",
            key_path.display()
        ));
    }

    let key = russh::keys::PrivateKey::read_openssh_file(key_path)
        .map_err(|e| anyhow::anyhow!("Failed to read host key: {}", e))?;

    Ok(key)
}
