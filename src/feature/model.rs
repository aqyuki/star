use std::time::Instant;

use log::info;

pub struct Timer {
    start: Instant,
    name: String,
}

impl Timer {
    pub fn new(name: &str) -> Timer {
        Timer {
            start: Instant::now(),
            name: name.to_string(),
        }
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        let elapsed = self.start.elapsed();
        info!(
            "{} took {}s {}ms",
            self.name,
            elapsed.as_secs(),
            elapsed.subsec_millis()
        );
    }
}
