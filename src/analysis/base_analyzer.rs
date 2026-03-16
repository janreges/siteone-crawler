// SiteOne Crawler - BaseAnalyzer
// (c) Jan Reges <jan.reges@siteone.cz>

use std::collections::HashMap;
use std::time::Instant;

/// Common state and methods shared by all analyzers.
/// Embed this as a field in each concrete analyzer struct.
#[derive(Debug, Default)]
pub struct BaseAnalyzer {
    /// Total exec times of analyzer methods: "ClassName::method" -> seconds
    pub exec_times: HashMap<String, f64>,
    /// Total exec counts of analyzer methods: "ClassName::method" -> count
    pub exec_counts: HashMap<String, usize>,
}

impl BaseAnalyzer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Measure and increment exec time and count of an analyzer method.
    pub fn measure_exec_time(&mut self, class: &str, method: &str, start_time: Instant) {
        let elapsed = start_time.elapsed().as_secs_f64();
        let key = format!("{}::{}", class, method);

        *self.exec_times.entry(key.clone()).or_insert(0.0) += elapsed;
        *self.exec_counts.entry(key).or_insert(0) += 1;
    }

    pub fn get_exec_times(&self) -> &HashMap<String, f64> {
        &self.exec_times
    }

    pub fn get_exec_counts(&self) -> &HashMap<String, usize> {
        &self.exec_counts
    }
}
