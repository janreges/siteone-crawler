// SiteOne Crawler - ProgressReporter (throttled progress lines)
// (c) Jan Reges <jan.reges@siteone.cz>
//

use std::time::{Duration, Instant};

/// Condenses the per-URL table into one line per interval (`--progress-interval`), so the log of
/// a long crawl stays small (CI systems cut long job logs), e.g.
/// `Progress: 22232/29504 (75%) | 31 URLs/s | avg 70 ms | 2xx 22000, 3xx 100, 4xx 120, 5xx 2, err 10 | 00:12:03`.
/// The clock is passed in, so the throttling is testable without sleeping.
pub struct ProgressReporter {
    interval: Duration,
    started: Instant,
    last_line: Instant,
    /// URLs recorded so far. Rows can arrive out of order, so their own `done` numbers are not used.
    done: usize,
    /// `done` when the last line was printed, so the final line is not a repeat of it
    done_at_last_line: usize,
    /// Highest `total` seen (the queue grows while crawling)
    total: usize,
    request_time_sum: f64,
    status_2xx: usize,
    status_3xx: usize,
    status_4xx: usize,
    status_5xx: usize,
    /// Every status outside 200-599: negative codes (connection error, timeout, skipped) and others
    errors: usize,
}

impl ProgressReporter {
    pub fn new(interval: Duration, now: Instant) -> Self {
        Self {
            interval,
            started: now,
            last_line: now,
            done: 0,
            done_at_last_line: 0,
            total: 0,
            request_time_sum: 0.0,
            status_2xx: 0,
            status_3xx: 0,
            status_4xx: 0,
            status_5xx: 0,
            errors: 0,
        }
    }

    /// Record one crawled URL (`progress_status` is the `done/total` text of the URL table).
    /// Returns a line when at least one interval has passed since the previous line.
    pub fn record(&mut self, status: i32, elapsed_time: f64, progress_status: &str, now: Instant) -> Option<String> {
        self.done += 1;
        if let Some(total) = progress_status
            .split_once('/')
            .and_then(|(_, total)| total.trim().parse::<usize>().ok())
        {
            self.total = self.total.max(total);
        }
        self.request_time_sum += elapsed_time;
        match status {
            200..=299 => self.status_2xx += 1,
            300..=399 => self.status_3xx += 1,
            400..=499 => self.status_4xx += 1,
            500..=599 => self.status_5xx += 1,
            _ => self.errors += 1,
        }

        if now.duration_since(self.last_line) < self.interval {
            return None;
        }
        self.last_line = now;
        self.done_at_last_line = self.done;
        Some(self.line(now))
    }

    /// The closing line after the crawl; `None` when no URL was recorded since the last line.
    pub fn finish(&self, now: Instant) -> Option<String> {
        (self.done > self.done_at_last_line).then(|| self.line(now))
    }

    fn line(&self, now: Instant) -> String {
        let elapsed = now.duration_since(self.started);
        let seconds = elapsed.as_secs_f64();
        let total = self.total.max(self.done);
        let percent = (self.done * 100).checked_div(total).unwrap_or(0);
        let urls_per_sec = if seconds > 0.0 { self.done as f64 / seconds } else { 0.0 };
        // One decimal for slow (rate-limited) crawls, which would otherwise show `0 URLs/s`.
        let urls_per_sec = if urls_per_sec < 10.0 {
            format!("{urls_per_sec:.1}")
        } else {
            format!("{}", urls_per_sec.round() as u64)
        };
        let avg_ms = (self.request_time_sum * 1000.0 / self.done as f64).round() as u64;
        let clock = elapsed.as_secs();
        format!(
            "Progress: {}/{} ({}%) | {} URLs/s | avg {} ms | 2xx {}, 3xx {}, 4xx {}, 5xx {}, err {} | {:02}:{:02}:{:02}",
            self.done,
            total,
            percent,
            urls_per_sec,
            avg_ms,
            self.status_2xx,
            self.status_3xx,
            self.status_4xx,
            self.status_5xx,
            self.errors,
            clock / 3600,
            clock % 3600 / 60,
            clock % 60
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_matches_the_documented_format() {
        let t0 = Instant::now();
        let mut progress = ProgressReporter::new(Duration::from_secs(3600), t0);
        let mut done = 0;
        for (status, count) in [(200, 22000), (301, 100), (404, 120), (500, 2), (-1, 10)] {
            for _ in 0..count {
                done += 1;
                assert_eq!(progress.record(status, 0.07, &format!("{done}/29504"), t0), None);
            }
        }
        assert_eq!(
            progress.finish(t0 + Duration::from_secs(723)).as_deref(),
            Some(
                "Progress: 22232/29504 (75%) | 31 URLs/s | avg 70 ms | 2xx 22000, 3xx 100, 4xx 120, 5xx 2, err 10 | 00:12:03"
            )
        );
    }

    #[test]
    fn prints_one_line_per_interval() {
        let t0 = Instant::now();
        let at = |secs: u64| t0 + Duration::from_secs(secs);
        let mut progress = ProgressReporter::new(Duration::from_secs(10), t0);
        assert_eq!(progress.record(200, 0.1, "1/10", at(3)), None);
        assert_eq!(progress.record(200, 0.1, "2/10", at(9)), None);
        assert_eq!(
            progress.record(200, 0.1, "3/10", at(10)).as_deref(),
            Some("Progress: 3/10 (30%) | 0.3 URLs/s | avg 100 ms | 2xx 3, 3xx 0, 4xx 0, 5xx 0, err 0 | 00:00:10")
        );
        assert_eq!(progress.record(404, 0.1, "4/10", at(19)), None);
        assert!(progress.record(200, 0.1, "5/10", at(20)).is_some());
    }

    #[test]
    fn rows_arriving_out_of_order_still_add_up() {
        let t0 = Instant::now();
        let mut progress = ProgressReporter::new(Duration::from_secs(10), t0);
        assert_eq!(progress.record(200, 0.1, "2/2", t0), None);
        assert_eq!(progress.record(200, 0.1, "1/2", t0), None);
        let line = progress.finish(t0 + Duration::from_secs(1)).unwrap();
        assert!(line.starts_with("Progress: 2/2 (100%) | "), "{line}");
    }

    #[test]
    fn final_line_needs_a_recorded_url() {
        let t0 = Instant::now();
        let mut progress = ProgressReporter::new(Duration::from_secs(10), t0);
        assert_eq!(progress.finish(t0), None);
        assert_eq!(progress.record(-2, 1.5, "1/1", t0), None);
        assert_eq!(
            progress.finish(t0 + Duration::from_secs(1)).as_deref(),
            Some("Progress: 1/1 (100%) | 1.0 URLs/s | avg 1500 ms | 2xx 0, 3xx 0, 4xx 0, 5xx 0, err 1 | 00:00:01")
        );
    }

    #[test]
    fn final_line_is_not_a_repeat_of_the_last_line() {
        let t0 = Instant::now();
        let at = |secs: u64| t0 + Duration::from_secs(secs);
        let mut progress = ProgressReporter::new(Duration::from_secs(10), t0);
        assert_eq!(progress.record(200, 0.1, "1/2", at(1)), None);
        assert!(progress.record(200, 0.1, "2/2", at(10)).is_some());
        assert_eq!(
            progress.finish(at(10)),
            None,
            "the last line already shows the final state"
        );

        assert_eq!(progress.record(200, 0.1, "3/3", at(11)), None);
        assert!(progress.finish(at(11)).unwrap().starts_with("Progress: 3/3 (100%) | "));
    }
}
