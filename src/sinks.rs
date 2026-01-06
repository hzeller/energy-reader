use std::time::{SystemTime,UNIX_EPOCH};

/// Result of the detection logic.
pub trait ResultSink {
    fn log_value(&self, time: SystemTime, number: u64);
    fn log_error(&self, time: SystemTime, err: String);
}

pub struct StdOutSink;
impl ResultSink for StdOutSink {
    fn log_value(&self, time: SystemTime, number: u64) {
        let ts = time.duration_since(UNIX_EPOCH).unwrap().as_secs();
        println!("{} {}", ts, number);
    }
    fn log_error(&self, time: SystemTime, err: String) {
        let ts = time.duration_since(UNIX_EPOCH).unwrap().as_secs();
        eprintln!("{} {}", ts, err);
    }
}

// TODO: Prometheus sink
