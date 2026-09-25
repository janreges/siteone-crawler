// SiteOne Crawler - Browser rendering subsystem
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Optional, opt-in browser rendering mode. Page diagnostics types and the screenshot
// viewport parser live here and are always compiled (so `HttpResponse` can carry an inert
// `Option<BrowserDiagnostics>` and options are validated in every build), while the actual
// Chromium driver lives behind the `browser` Cargo feature.

pub mod diagnostics;
pub mod viewport;

#[cfg(feature = "browser")]
mod auto_scroll;
#[cfg(feature = "browser")]
pub mod cookie_consent;
#[cfg(feature = "browser")]
pub mod launcher;
#[cfg(feature = "browser")]
mod renderer;
#[cfg(feature = "browser")]
pub mod screenshot;
#[cfg(feature = "browser")]
pub use renderer::BrowserRenderer;
