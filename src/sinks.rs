use std::time::{SystemTime,UNIX_EPOCH};

/// Result of the detection logic.
pub trait ResultSink {
    fn log_value(&self, time: SystemTime, number: u64);
    fn log_error(&self, time: SystemTime, err: &str);
}

pub struct StdOutSink;
impl StdOutSink {
    fn convert_ts(time: SystemTime) -> u64 {
        time.duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}
impl ResultSink for StdOutSink {
    fn log_value(&self, time: SystemTime, number: u64) {
        println!("{} {}", Self::convert_ts(time), number);
    }
    fn log_error(&self, time: SystemTime, err: &str) {
        eprintln!("{} {}", Self::convert_ts(time), err);
    }
}

// TODO: Prometheus sink
