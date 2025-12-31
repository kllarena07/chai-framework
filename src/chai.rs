use ratatui::Frame;

pub trait ChaiApp: Copy + Clone {
    fn new() -> Self;
    fn update(&mut self);
    fn draw(&mut self, f: &mut Frame);
    fn tick(&mut self, f: &mut Frame) {
        self.update();
        self.draw(f);
    }
    fn handle_input(&mut self, data: &[u8]);
}
