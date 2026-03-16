// SiteOne Crawler - Quality Score data model
// (c) Jan Reges <jan.reges@siteone.cz>

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityScores {
    pub overall: CategoryScore,
    pub categories: Vec<CategoryScore>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryScore {
    pub name: String,
    pub code: String,
    pub score: f64,
    pub label: String,
    pub weight: f64,
    pub deductions: Vec<Deduction>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Deduction {
    pub reason: String,
    pub points: f64,
}

impl CategoryScore {
    pub fn color_hex(&self) -> &'static str {
        match self.score {
            s if s >= 9.0 => "#22c55e",
            s if s >= 7.0 => "#3b82f6",
            s if s >= 5.0 => "#eab308",
            s if s >= 3.0 => "#a855f7",
            _ => "#ef4444",
        }
    }

    pub fn console_color(&self) -> &'static str {
        match self.score {
            s if s >= 9.0 => "green",
            s if s >= 7.0 => "blue",
            s if s >= 5.0 => "yellow",
            s if s >= 3.0 => "magenta",
            _ => "red",
        }
    }
}

pub fn score_label(score: f64) -> &'static str {
    match score {
        s if s >= 9.0 => "Excellent",
        s if s >= 7.0 => "Good",
        s if s >= 5.0 => "Fair",
        s if s >= 3.0 => "Poor",
        _ => "Critical",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_score(score: f64) -> CategoryScore {
        CategoryScore {
            name: "Test".to_string(),
            code: "test".to_string(),
            score,
            label: score_label(score).to_string(),
            weight: 1.0,
            deductions: Vec::new(),
        }
    }

    #[test]
    fn score_label_values() {
        assert_eq!(score_label(0.0), "Critical");
        assert_eq!(score_label(3.0), "Poor");
        assert_eq!(score_label(5.0), "Fair");
        assert_eq!(score_label(7.0), "Good");
        assert_eq!(score_label(9.0), "Excellent");
    }

    #[test]
    fn score_label_boundaries() {
        assert_eq!(score_label(2.99), "Critical");
        assert_eq!(score_label(4.99), "Poor");
        assert_eq!(score_label(6.99), "Fair");
        assert_eq!(score_label(8.99), "Good");
    }

    #[test]
    fn color_hex_green_for_excellent() {
        assert_eq!(make_score(9.5).color_hex(), "#22c55e");
    }

    #[test]
    fn color_hex_purple_for_poor() {
        assert_eq!(make_score(4.0).color_hex(), "#a855f7");
    }

    #[test]
    fn color_hex_red_for_critical() {
        assert_eq!(make_score(1.0).color_hex(), "#ef4444");
    }

    #[test]
    fn color_hex_boundaries() {
        assert_eq!(make_score(8.99).color_hex(), "#3b82f6");
        assert_eq!(make_score(6.99).color_hex(), "#eab308");
    }

    #[test]
    fn console_color_values() {
        assert_eq!(make_score(9.5).console_color(), "green");
        assert_eq!(make_score(7.5).console_color(), "blue");
        assert_eq!(make_score(5.5).console_color(), "yellow");
        assert_eq!(make_score(3.5).console_color(), "magenta");
        assert_eq!(make_score(1.0).console_color(), "red");
    }
}
