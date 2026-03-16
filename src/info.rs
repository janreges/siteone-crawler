// SiteOne Crawler - Info
// (c) Jan Reges <jan.reges@siteone.cz>

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Info {
    pub name: String,
    pub version: String,
    pub executed_at: String,
    pub command: String,
    pub hostname: String,
    pub final_user_agent: String,
    /// The initial URL passed via --url option
    pub initial_url: String,
}

impl Info {
    pub fn new(
        name: String,
        version: String,
        executed_at: String,
        command: String,
        hostname: String,
        final_user_agent: String,
        initial_url: String,
    ) -> Self {
        Self {
            name,
            version,
            executed_at,
            command,
            hostname,
            final_user_agent,
            initial_url,
        }
    }

    pub fn set_final_user_agent(&mut self, final_user_agent: String) {
        self.final_user_agent = final_user_agent;
    }
}
