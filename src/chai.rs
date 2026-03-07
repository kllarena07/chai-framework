use ratatui::Frame;

pub trait ChaiApp {
    fn new() -> Self;
    fn update(&mut self);
    fn draw(&mut self, f: &mut Frame);
    fn handle_input(&mut self, data: &[u8]);
    /// Return true to signal that the SSH connection should be closed.
    /// The default implementation always returns false.
    fn should_quit(&self) -> bool {
        false
    }
    // ctrl c
    fn capture_ctrl_c(&self) -> bool {
        false
    }
}
