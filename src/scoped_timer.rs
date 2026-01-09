
use std::time::Instant;

pub struct ScopedTimer {
    title: &'static str,
    start: Instant,
}

impl ScopedTimer {
    pub fn new(title: &'static str) -> ScopedTimer {
        ScopedTimer{
            title,
            start: Instant::now(),
        }
    }
}

impl Drop for ScopedTimer {
    fn drop(&mut self) {
        let duration = Instant::now() - self.start;
        println!("{} took {:?}", self.title, duration);
    }
}
