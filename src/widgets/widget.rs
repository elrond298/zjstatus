use crate::config::ZellijState;

pub trait Widget {
    fn process(&self, name: &str, state: &ZellijState) -> String;
    fn process_at_level(&self, name: &str, state: &ZellijState, _level: usize) -> String {
        self.process(name, state)
    }
    fn process_click(&self, name: &str, state: &ZellijState, pos: usize);
    fn process_click_at_level(&self, name: &str, state: &ZellijState, pos: usize, _level: usize) {
        self.process_click(name, state, pos);
    }
}
