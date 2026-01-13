use std::time::{SystemTime, UNIX_EPOCH};

/// Result receiver of the detection logic.
pub trait ResultSink {
    fn log_value(&mut self, time: SystemTime, number: u64);
    fn log_error(&mut self, time: SystemTime, err: &str);
}

fn convert_ts(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// A sink that only accepts values that look plausible: always increasing
/// and not exceeding a increase rate.
pub struct PlausibilityFilterSink {
    last_value: u64,
    last_timestamp: u64,
    max_plausible_rate: f32, // value / sec
    delegatee: Box<dyn ResultSink>,
}

impl PlausibilityFilterSink {
    /// Create new stdout sink, that only emits values that are ever increasing,
    /// and also don't increase by more than max_plausible_rate (value/sec)
    pub fn new(max_plausible_rate: f32, delegatee: Box<dyn ResultSink>) -> Self {
        PlausibilityFilterSink {
            last_value: 0,
            last_timestamp: 0,
            max_plausible_rate,
            delegatee,
        }
    }
}

impl ResultSink for PlausibilityFilterSink {
    fn log_value(&mut self, time: SystemTime, number: u64) {
        let ts = convert_ts(time);

        // Not going backwards ?
        if number < self.last_value {
            let err = format!(
                "Value {} going backwards (before: {})",
                number, self.last_value
            );
            self.log_error(time, &err);
            return;
        }

        // Within plausible rate ?
        if self.last_timestamp > 0 {
            let delta_v = number - self.last_value;
            let delta_t = ts.saturating_sub(self.last_timestamp);

            if delta_t > 0 {
                let rate = delta_v as f32 / delta_t as f32;
                if rate > self.max_plausible_rate {
                    let err = format!(
                        "Exceeded max plausible rate: {} -> {} in {}s (rate: {:.3}/s, max: {:.3}/s)",
                        self.last_value, number, delta_t, rate, self.max_plausible_rate
                    );
                    self.log_error(time, &err);
                    return;
                }
            }
        }

        self.delegatee.log_value(time, number);
        self.last_value = number;
        self.last_timestamp = ts;
    }

    fn log_error(&mut self, time: SystemTime, err: &str) {
        self.delegatee.log_error(time, err);
    }
}

/// A ResultSink that outputs timestamp + value on stdout, errors to stderr.
pub struct StdOutSink;
impl ResultSink for StdOutSink {
    fn log_value(&mut self, time: SystemTime, number: u64) {
        println!("{} {}", convert_ts(time), number);
    }
    fn log_error(&mut self, time: SystemTime, err: &str) {
        eprintln!("{} ERROR: {}", convert_ts(time), err);
    }
}
// TODO: Prometheus sink
