use chai::{ChaiApp, ChaiServer};
use ratatui::{
    Frame,
    style::{Color, Style},
    widgets::{Block, Borders, Clear, Paragraph},
};
use russh::{MethodKind, MethodSet, server::Config};
use std::env;
use std::path::Path;

#[derive(Copy, Clone)]
pub struct MyApp {
    pub counter: usize,
}

impl ChaiApp for MyApp {
    fn new() -> Self {
        Self { counter: 0 }
    }
    fn update(&mut self) {
        self.counter += 1;
    }
    fn draw(&mut self, f: &mut Frame) {
        let area = f.area();
        f.render_widget(Clear, area);
        let style = match self.counter % 3 {
            0 => Style::default().fg(Color::Red),
            1 => Style::default().fg(Color::Green),
            _ => Style::default().fg(Color::Blue),
        };
        let paragraph = Paragraph::new(format!("Counter: {}", self.counter))
            .alignment(ratatui::layout::Alignment::Center)
            .style(style);
        let block = Block::default()
            .title("Press 'c' to reset the counter!")
            .borders(Borders::ALL);
        f.render_widget(paragraph.block(block), area);
    }
    fn handle_input(&mut self, data: &[u8]) {
        if data == b"c" {
            self.counter = 0;
        }
    }
}

fn load_host_keys() -> Result<russh::keys::PrivateKey, anyhow::Error> {
    let hk_loc = env::var("HK_LOC").expect("HK_LOC was not defined.");
    let key_path = Path::new(&hk_loc);

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

#[tokio::main]
async fn main() {
    let host_key = load_host_keys().expect("Failed to load host keys");
    let mut methods = MethodSet::empty();
    methods.push(MethodKind::None);

    let config = Config {
        inactivity_timeout: Some(std::time::Duration::from_secs(3600)),
        auth_rejection_time: std::time::Duration::from_secs(3),
        auth_rejection_time_initial: Some(std::time::Duration::from_secs(0)),
        keys: vec![host_key],
        methods,
        ..Default::default()
    };

    let mut server = ChaiServer::<MyApp>::new(2222);
    server.run(config).await.expect("Failed running server");
}
