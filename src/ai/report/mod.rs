// SiteOne Crawler - AI report engine
// (c) Jan Reges <jan.reges@siteone.cz>
//
// First-class AI reports: a generic typed per-page extraction model + presets, consumed by the
// JSON and HTML exporters. `custom` (findings array) stays separate; this is the columnar flow.

pub mod coerce;
pub mod compliance;
pub mod ia;
pub mod locale;
pub mod model;
pub mod presets;
pub mod schema;
pub mod synthesis;
