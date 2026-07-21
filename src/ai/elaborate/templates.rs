// SiteOne Crawler - brand elaborate: templates & document skeletons
// (c) Jan Reges <jan.reges@siteone.cz>
//
// A template picks the extraction FOCUS and the fixed document SKELETON. The skeleton is a stable
// ordered list of sections; the LLM only writes the `Prose` sections, the crawler assembles the
// `List`/`Sources` sections from the deduped `BrandModel`. Fixing the skeleton keeps every
// elaborate "cleverly structured" and consistent across sites; empty sections are simply omitted.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Template {
    Corporate,
    Personal,
    Product,
}

impl Template {
    pub fn key(&self) -> &'static str {
        match self {
            Template::Corporate => "corporate",
            Template::Personal => "personal",
            Template::Product => "product",
        }
    }
}

/// Resolve the effective template. An explicit CLI value always wins; `auto` maps the LLM-detected
/// `site_type` (corporate/ecommerce/other → Corporate; personal → Personal; product → Product).
pub fn resolve(cli_value: &str, site_type: &str) -> Template {
    match cli_value.trim().to_ascii_lowercase().as_str() {
        "corporate" => Template::Corporate,
        "personal" => Template::Personal,
        "product" => Template::Product,
        _ => match site_type.trim().to_ascii_lowercase().as_str() {
            "personal" => Template::Personal,
            "product" => Template::Product,
            _ => Template::Corporate, // corporate | ecommerce | other | unknown
        },
    }
}

/// Where a crawler-rendered list draws its rows from in the `BrandModel`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListSource {
    Services,
    Products,
    Offerings,
    People,
    Locations,
    Facts,
    Channels,
    Quotes,
    Catalogs,
}

/// What produces a section's body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SectionKind {
    /// LLM-written connective prose for this section id.
    Prose,
    /// Crawler-rendered list from the merged model (un-hallucinatable).
    List(ListSource),
    /// Crawler-rendered `## Sources` provenance list.
    Sources,
}

/// One section of the fixed skeleton.
pub struct SectionSpec {
    /// Stable id (also the key of the prose the synthesis LLM returns).
    pub id: &'static str,
    /// English default heading (localized in `doc.rs`).
    pub heading: &'static str,
    pub kind: SectionKind,
    /// One-line instruction for the synthesis prompt (used for `Prose` sections).
    pub instruction: &'static str,
}

const CORPORATE: &[SectionSpec] = &[
    SectionSpec {
        id: "abstract",
        heading: "Executive summary",
        kind: SectionKind::Prose,
        instruction: "A one-paragraph executive summary of who the company is and what it does.",
    },
    SectionSpec {
        id: "identity_mission",
        heading: "Identity & mission",
        kind: SectionKind::Prose,
        instruction: "The company's identity, mission, values and positioning.",
    },
    SectionSpec {
        id: "offer_intro",
        heading: "What they offer",
        kind: SectionKind::Prose,
        instruction: "A short narrative introducing the overall offering (services vs products).",
    },
    SectionSpec {
        id: "services",
        heading: "Services",
        kind: SectionKind::List(ListSource::Services),
        instruction: "",
    },
    SectionSpec {
        id: "products",
        heading: "Products",
        kind: SectionKind::List(ListSource::Products),
        instruction: "",
    },
    SectionSpec {
        id: "catalogs",
        heading: "Catalogs & collections",
        kind: SectionKind::List(ListSource::Catalogs),
        instruction: "",
    },
    SectionSpec {
        id: "people",
        heading: "People & roles",
        kind: SectionKind::List(ListSource::People),
        instruction: "",
    },
    SectionSpec {
        id: "locations",
        heading: "Locations & branches",
        kind: SectionKind::List(ListSource::Locations),
        instruction: "",
    },
    SectionSpec {
        id: "facts",
        heading: "Key facts & numbers",
        kind: SectionKind::List(ListSource::Facts),
        instruction: "",
    },
    SectionSpec {
        id: "audiences",
        heading: "Audiences & who it's for",
        kind: SectionKind::Prose,
        instruction: "The primary audiences and customers the company serves.",
    },
    SectionSpec {
        id: "channels",
        heading: "Communication channels",
        kind: SectionKind::List(ListSource::Channels),
        instruction: "",
    },
    SectionSpec {
        id: "statements",
        heading: "Notable statements",
        kind: SectionKind::List(ListSource::Quotes),
        instruction: "",
    },
    SectionSpec {
        id: "sources",
        heading: "Sources",
        kind: SectionKind::Sources,
        instruction: "",
    },
];

const PERSONAL: &[SectionSpec] = &[
    SectionSpec {
        id: "abstract",
        heading: "Summary",
        kind: SectionKind::Prose,
        instruction: "A one-paragraph summary of who this person is and what they do.",
    },
    SectionSpec {
        id: "identity_bio",
        heading: "Identity & bio",
        kind: SectionKind::Prose,
        instruction: "The person's biography, background and current focus.",
    },
    SectionSpec {
        id: "expertise",
        heading: "Expertise & skills",
        kind: SectionKind::Prose,
        instruction: "The person's areas of expertise and skills.",
    },
    SectionSpec {
        id: "work",
        heading: "Work & portfolio",
        kind: SectionKind::List(ListSource::Offerings),
        instruction: "",
    },
    SectionSpec {
        id: "experience",
        heading: "Experience",
        kind: SectionKind::Prose,
        instruction: "Notable roles, projects and experience.",
    },
    SectionSpec {
        id: "contact",
        heading: "Contact & channels",
        kind: SectionKind::List(ListSource::Channels),
        instruction: "",
    },
    SectionSpec {
        id: "people",
        heading: "Profiles",
        kind: SectionKind::List(ListSource::People),
        instruction: "",
    },
    SectionSpec {
        id: "statements",
        heading: "Notable statements",
        kind: SectionKind::List(ListSource::Quotes),
        instruction: "",
    },
    SectionSpec {
        id: "sources",
        heading: "Sources",
        kind: SectionKind::Sources,
        instruction: "",
    },
];

const PRODUCT: &[SectionSpec] = &[
    SectionSpec {
        id: "abstract",
        heading: "Summary",
        kind: SectionKind::Prose,
        instruction: "A one-paragraph summary of the product and what problem it solves.",
    },
    SectionSpec {
        id: "product_identity",
        heading: "Product identity",
        kind: SectionKind::Prose,
        instruction: "What the product is, its category and its core purpose.",
    },
    SectionSpec {
        id: "value_prop",
        heading: "Value proposition",
        kind: SectionKind::Prose,
        instruction: "The value proposition and key differentiators.",
    },
    SectionSpec {
        id: "features",
        heading: "Features & capabilities",
        kind: SectionKind::List(ListSource::Offerings),
        instruction: "",
    },
    SectionSpec {
        id: "audiences",
        heading: "Use cases & audiences",
        kind: SectionKind::Prose,
        instruction: "The primary use cases and target audiences.",
    },
    SectionSpec {
        id: "facts",
        heading: "Pricing & key facts",
        kind: SectionKind::List(ListSource::Facts),
        instruction: "",
    },
    SectionSpec {
        id: "proof",
        heading: "Proof & credibility",
        kind: SectionKind::List(ListSource::Quotes),
        instruction: "",
    },
    SectionSpec {
        id: "channels",
        heading: "Resources & channels",
        kind: SectionKind::List(ListSource::Channels),
        instruction: "",
    },
    SectionSpec {
        id: "sources",
        heading: "Sources",
        kind: SectionKind::Sources,
        instruction: "",
    },
];

pub fn skeleton(t: Template) -> &'static [SectionSpec] {
    match t {
        Template::Corporate => CORPORATE,
        Template::Personal => PERSONAL,
        Template::Product => PRODUCT,
    }
}

/// The prose section ids the synthesis LLM must fill for a template (in order).
pub fn prose_ids(t: Template) -> Vec<&'static str> {
    skeleton(t)
        .iter()
        .filter(|s| s.kind == SectionKind::Prose)
        .map(|s| s.id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_explicit_wins_over_site_type() {
        assert_eq!(resolve("auto", "ecommerce"), Template::Corporate);
        assert_eq!(resolve("auto", "personal"), Template::Personal);
        assert_eq!(resolve("auto", "product"), Template::Product);
        assert_eq!(resolve("product", "personal"), Template::Product); // explicit wins
        assert_eq!(resolve("corporate", ""), Template::Corporate);
        assert_eq!(resolve("weird", "unknown"), Template::Corporate); // fallback
    }

    #[test]
    fn every_skeleton_ends_with_sources_and_has_prose() {
        for t in [Template::Corporate, Template::Personal, Template::Product] {
            let sk = skeleton(t);
            assert_eq!(sk.last().unwrap().kind, SectionKind::Sources);
            assert!(sk.iter().filter(|s| s.kind == SectionKind::Prose).count() >= 3);
            // Ids are unique.
            let mut ids: Vec<&str> = sk.iter().map(|s| s.id).collect();
            ids.sort_unstable();
            let before = ids.len();
            ids.dedup();
            assert_eq!(before, ids.len(), "duplicate section id in {:?}", t);
        }
        assert!(prose_ids(Template::Corporate).contains(&"identity_mission"));
    }
}
