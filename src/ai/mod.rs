// SiteOne Crawler - AI subsystem
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Optional AI features: provider-agnostic LLM client over the existing reqwest stack,
// per-page analyses (SEO, llms.txt, typos, custom prompt), with strict cost controls.
// Nothing here runs unless the user explicitly configures an AI provider.

pub mod actions;
pub mod client;
pub mod config;
pub mod normalize;
pub mod page;
pub mod prompt;
pub mod provider;
pub mod runner;
pub mod secret;
pub mod selection;
pub mod summary;
pub mod usage;
