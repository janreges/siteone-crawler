// SiteOne Crawler - Badge for HTML Report
// (c) Jan Reges <jan.reges@siteone.cz>

/// Badge colors used in HTML report tabs and content
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BadgeColor {
    Red,
    Orange,
    Green,
    Blue,
    Neutral,
}

impl BadgeColor {
    pub fn as_css_class(&self) -> &'static str {
        match self {
            BadgeColor::Red => "red",
            BadgeColor::Orange => "orange",
            BadgeColor::Green => "green",
            BadgeColor::Blue => "blue",
            BadgeColor::Neutral => "neutral",
        }
    }
}

/// Badge displayed in tab titles or content to show counts/status
#[derive(Debug, Clone)]
pub struct Badge {
    pub value: String,
    pub color: BadgeColor,
    pub title: Option<String>,
}

impl Badge {
    pub fn new(value: String, color: BadgeColor) -> Self {
        Self {
            value,
            color,
            title: None,
        }
    }

    pub fn with_title(value: String, color: BadgeColor, title: &str) -> Self {
        Self {
            value,
            color,
            title: Some(title.to_string()),
        }
    }
}
