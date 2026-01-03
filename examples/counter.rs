use chai_framework::{ChaiApp, ChaiServer, load_host_keys};
use ratatui::{
    Frame,
    style::{Color, Style},
    widgets::{Block, Borders, Clear, Paragraph},
};
use russh::{MethodKind, MethodSet, server::Config};

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

#[tokio::main]
async fn main() {
    let host_key =
        load_host_keys("./examples/authorized_keys/id_ed25519").expect("Failed to load host keys");
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
