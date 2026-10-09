mod queue;
pub mod render;

use queue::Queue;

pub struct Lang {
    pub code: String,
    pub feeds: bool,
}

pub struct Site {
    pub langs: Vec<Lang>,
}

impl Site {
    pub fn build(&self) -> Vec<String> {
        Queue::full_build(self).outputs()
    }
}
