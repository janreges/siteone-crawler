// SiteOne Crawler - BasicStats
// (c) Jan Reges <jan.reges@siteone.cz>

use std::collections::BTreeMap;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::result::visited_url::VisitedUrl;
use crate::types::ContentTypeId;
use crate::utils;

/// 90th-percentile value of a slice (nearest-rank). Returns 0.0 for an empty slice.
fn percentile(values: &mut [f64], p: usize) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let rank = ((p as f64 / 100.0) * (values.len() - 1) as f64).round() as usize;
    (values[rank.min(values.len() - 1)] * 1000.0).round() / 1000.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BasicStats {
    pub total_execution_time: f64,
    pub total_urls: usize,
    pub total_size: i64,
    pub total_size_formatted: String,
    pub total_requests_times: f64,
    pub total_requests_times_avg: f64,
    pub total_requests_times_min: f64,
    pub total_requests_times_max: f64,
    /// 90th-percentile response time of HTML documents (a fairer "what users feel" proxy than the
    /// all-asset average, which is skewed by large images/videos). Falls back to all URLs if no HTML.
    pub total_requests_times_p90: f64,
    pub count_by_status: BTreeMap<i32, usize>,
    pub count_by_content_type: BTreeMap<i32, usize>,
}

impl BasicStats {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        total_execution_time: f64,
        total_urls: usize,
        total_size: i64,
        total_size_formatted: String,
        total_requests_times: f64,
        total_requests_times_avg: f64,
        total_requests_times_min: f64,
        total_requests_times_max: f64,
        total_requests_times_p90: f64,
        count_by_status: BTreeMap<i32, usize>,
        count_by_content_type: BTreeMap<i32, usize>,
    ) -> Self {
        Self {
            total_execution_time,
            total_urls,
            total_size,
            total_size_formatted,
            total_requests_times,
            total_requests_times_avg,
            total_requests_times_min,
            total_requests_times_max,
            total_requests_times_p90,
            count_by_status,
            count_by_content_type,
        }
    }

    pub fn from_visited_urls(visited_urls: &[&VisitedUrl], start_time: Instant) -> Self {
        let total_urls = visited_urls.len();
        let mut total_size: i64 = 0;
        let mut total_time: f64 = 0.0;
        let mut min_time: Option<f64> = None;
        let mut max_time: Option<f64> = None;
        let mut count_by_status: BTreeMap<i32, usize> = BTreeMap::new();
        let mut count_by_content_type: BTreeMap<i32, usize> = BTreeMap::new();

        for url in visited_urls {
            total_time += url.request_time;
            total_size += url.size.unwrap_or(0);
            *count_by_status.entry(url.status_code).or_insert(0) += 1;
            *count_by_content_type.entry(url.content_type as i32).or_insert(0) += 1;
            min_time = Some(match min_time {
                Some(current) => current.min(url.request_time),
                None => url.request_time,
            });
            max_time = Some(match max_time {
                Some(current) => current.max(url.request_time),
                None => url.request_time,
            });
        }

        let total_execution_time = (start_time.elapsed().as_secs_f64() * 1000.0).round() / 1000.0;
        let total_requests_times = (total_time * 1000.0).round() / 1000.0;
        let total_requests_times_avg = if total_urls > 0 {
            (total_time / total_urls as f64 * 1000.0).round() / 1000.0
        } else {
            0.0
        };
        let total_requests_times_min = (min_time.unwrap_or(0.0) * 1000.0).round() / 1000.0;
        let total_requests_times_max = (max_time.unwrap_or(0.0) * 1000.0).round() / 1000.0;

        // p90 over HTML documents (fairer "what users feel" proxy); fall back to all URLs if none.
        let mut html_times: Vec<f64> = visited_urls
            .iter()
            .filter(|u| u.content_type == ContentTypeId::Html && u.status_code == 200)
            .map(|u| u.request_time)
            .collect();
        if html_times.is_empty() {
            html_times = visited_urls.iter().map(|u| u.request_time).collect();
        }
        let total_requests_times_p90 = percentile(&mut html_times, 90);

        Self {
            total_execution_time,
            total_urls,
            total_size,
            total_size_formatted: utils::get_formatted_size(total_size, 0),
            total_requests_times,
            total_requests_times_avg,
            total_requests_times_min,
            total_requests_times_max,
            total_requests_times_p90,
            count_by_status,
            count_by_content_type,
        }
    }

    pub fn get_as_html(&self) -> String {
        let mut html = String::from("<table class=\"table table-bordered table-striped table-hover\">");
        html.push_str("<tr><th colspan=\"2\">Basic stats</th></tr>");
        html.push_str(&format!(
            "<tr><td>Total execution time</td><td>{}</td></tr>",
            utils::get_formatted_duration(self.total_execution_time)
        ));
        html.push_str(&format!("<tr><td>Total URLs</td><td>{}</td></tr>", self.total_urls));
        html.push_str(&format!(
            "<tr><td>Total size</td><td>{}</td></tr>",
            self.total_size_formatted
        ));
        html.push_str(&format!(
            "<tr><td>Requests - total time</td><td>{}</td></tr>",
            utils::get_formatted_duration(self.total_requests_times)
        ));
        html.push_str(&format!(
            "<tr><td>Requests - avg time</td><td>{}</td></tr>",
            utils::get_formatted_duration(self.total_requests_times_avg)
        ));
        html.push_str(&format!(
            "<tr><td>Requests - min time</td><td>{}</td></tr>",
            utils::get_formatted_duration(self.total_requests_times_min)
        ));
        html.push_str(&format!(
            "<tr><td>Requests - max time</td><td>{}</td></tr>",
            utils::get_formatted_duration(self.total_requests_times_max)
        ));
        html.push_str(&format!(
            "<tr><td>Requests - p90 time (HTML docs)</td><td>{}</td></tr>",
            utils::get_formatted_duration(self.total_requests_times_p90)
        ));
        html.push_str("<tr><td>Requests by status</td><td>");
        for (status_code, count) in &self.count_by_status {
            let colored = utils::get_colored_status_code(*status_code, 0);
            let colored_html = utils::convert_bash_colors_in_text_to_html(&colored);
            html.push_str(&format!("{}: {}<br>", colored_html, count));
        }
        html.push_str("</td></tr>");
        html.push_str("</table>");

        html
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentile_p90_nearest_rank() {
        let mut v: Vec<f64> = (1..=10).map(|n| n as f64).collect();
        assert_eq!(percentile(&mut v, 90), 9.0);
    }

    #[test]
    fn percentile_empty_is_zero() {
        let mut v: Vec<f64> = vec![];
        assert_eq!(percentile(&mut v, 90), 0.0);
    }

    #[test]
    fn percentile_single_value() {
        let mut v = vec![0.42];
        assert_eq!(percentile(&mut v, 90), 0.42);
    }
}
