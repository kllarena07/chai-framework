use chai::{ChaiApp, ChaiServer};
use ratatui::{
    Frame,
    style::{Color, Style},
    widgets::{Block, Borders, Clear, Paragraph},
};

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
    let mut server = ChaiServer::<MyApp>::new(2222);
    server.run().await.expect("Failed running server");
}
