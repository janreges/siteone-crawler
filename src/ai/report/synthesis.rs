use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use super::locale::ReportLocale;
use super::model::{AiReportModel, ReportCell};

pub fn build_topic_analysis(report: &mut AiReportModel, report_language: &str) -> Value {
    let czech = ReportLocale::new(report_language).is_czech();
    let mut variants: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    for row in report.rows.iter().filter(|row| row.error.is_none()) {
        if let Some(ReportCell::Str(cluster)) = row.cells.get("topicCluster") {
            *variants
                .entry(normalize_label(cluster))
                .or_default()
                .entry(cluster.trim().to_string())
                .or_default() += 1;
        }
    }
    let canonical: BTreeMap<String, String> = variants
        .into_iter()
        .filter_map(|(key, values)| {
            values
                .into_iter()
                .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
                .map(|(display, _)| (key, display))
        })
        .collect();

    for row in report.rows.iter_mut().filter(|row| row.error.is_none()) {
        if let Some(ReportCell::Str(cluster)) = row.cells.get_mut("topicCluster")
            && let Some(display) = canonical.get(&normalize_label(cluster))
        {
            *cluster = display.clone();
        }
    }

    #[derive(Default)]
    struct Cluster {
        urls: BTreeSet<String>,
        topics: BTreeSet<String>,
        stages: BTreeSet<String>,
    }
    #[derive(Default)]
    struct Competing {
        cluster: String,
        topic: String,
        intent: String,
        audience: String,
        urls: BTreeSet<String>,
    }

    let mut clusters: BTreeMap<String, Cluster> = BTreeMap::new();
    let mut competing: BTreeMap<String, Competing> = BTreeMap::new();
    for row in report.rows.iter().filter(|row| row.error.is_none()) {
        let Some(ReportCell::Str(topic)) = row.cells.get("primaryTopic") else {
            continue;
        };
        let Some(ReportCell::Str(cluster)) = row.cells.get("topicCluster") else {
            continue;
        };
        let intent = match row.cells.get("searchIntent") {
            Some(ReportCell::Str(value)) => value.clone(),
            _ => String::new(),
        };
        let stage = match row.cells.get("funnelStage") {
            Some(ReportCell::Str(value)) => value.clone(),
            _ => String::new(),
        };
        let audience = match row.cells.get("targetAudience") {
            Some(ReportCell::Str(value)) => value.clone(),
            _ => String::new(),
        };

        let aggregate = clusters.entry(cluster.clone()).or_default();
        aggregate.urls.insert(row.url.clone());
        aggregate.topics.insert(topic.clone());
        if !stage.is_empty() {
            aggregate.stages.insert(stage);
        }

        let key = format!(
            "{}|{}|{}|{}",
            normalize_label(cluster),
            normalize_label(topic),
            normalize_label(&intent),
            normalize_label(&audience)
        );
        let candidate = competing.entry(key).or_default();
        candidate.cluster = cluster.clone();
        candidate.topic = topic.clone();
        candidate.intent = intent;
        candidate.audience = audience;
        candidate.urls.insert(row.url.clone());
    }

    let cluster_json = clusters
        .iter()
        .map(|(name, cluster)| {
            json!({
                "cluster": name,
                "pageCount": cluster.urls.len(),
                "urls": cluster.urls.iter().collect::<Vec<_>>(),
                "topics": cluster.topics.iter().collect::<Vec<_>>(),
                "funnelStages": cluster.stages.iter().collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    let cannibalization = competing
        .into_values()
        .filter(|candidate| candidate.urls.len() > 1)
        .map(|candidate| {
            json!({
                "topic": candidate.topic,
                "cluster": candidate.cluster,
                "searchIntent": candidate.intent,
                "targetAudience": candidate.audience,
                "urls": candidate.urls.into_iter().collect::<Vec<_>>(),
                "reason": if czech {
                    "Více URL ve stejném klastru cílí na stejné normalizované hlavní téma, záměr a publikum; prověřte jejich odlišení a kanonické cílení."
                } else {
                    "Multiple URLs in the same cluster target the same normalized primary topic, intent, and audience; review differentiation and canonical targeting."
                },
            })
        })
        .collect::<Vec<_>>();
    let gaps = clusters
        .iter()
        .filter_map(|(name, cluster)| {
            // A cluster with zero or one observed stage does not establish that it is intended to
            // span a marketing funnel (legal/contact/support clusters commonly are not).
            if cluster.stages.len() < 2 {
                return None;
            }
            let missing = ["TOFU", "MOFU", "BOFU"]
                .into_iter()
                .filter(|stage| !cluster.stages.contains(*stage))
                .collect::<Vec<_>>();
            (!missing.is_empty()).then(|| {
                json!({
                    "cluster": name,
                    "missingFunnelStages": missing,
                    "supportingUrls": cluster.urls.iter().collect::<Vec<_>>(),
                    "reason": if czech {
                        "Žádná analyzovaná stránka v tomto klastru nebyla zařazena do těchto fází funnelu."
                    } else {
                        "No analyzed page in this cluster was classified for these funnel stages."
                    },
                })
            })
        })
        .collect::<Vec<_>>();
    let thin = clusters
        .iter()
        .filter(|(_, cluster)| cluster.urls.len() <= 1)
        .map(|(name, cluster)| json!({"cluster": name, "urls": cluster.urls.iter().collect::<Vec<_>>() }))
        .collect::<Vec<_>>();

    json!({
        "type": "topicAnalysis",
        "methodology": if czech {
            "Deterministická analýza interního pokrytí pouze ve vybraných stránkách; nevyvozuje externí poptávku ve vyhledávání, témata konkurence ani mezery v klíčových slovech na úrovni trhu."
        } else {
            "Deterministic internal coverage analysis of the selected pages only; it does not infer external search demand, competitor topics, or market-level keyword gaps."
        },
        "clusters": cluster_json,
        "cannibalizationCandidates": cannibalization,
        "internalCoverageGaps": gaps,
        "thinClusters": thin,
    })
}

fn normalize_label(value: &str) -> String {
    value
        .to_lowercase()
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::report::model::{AiReportModel, FieldSpec, FieldType, ReportCell, ReportRow, SiteMeta};
    use serde_json::json;

    fn row(url: &str, topic: &str, cluster: &str, intent: &str, stage: &str) -> ReportRow {
        ReportRow::ok(
            url.to_string(),
            url.to_string(),
            [
                ("primaryTopic".to_string(), ReportCell::Str(topic.to_string())),
                ("topicCluster".to_string(), ReportCell::Str(cluster.to_string())),
                ("searchIntent".to_string(), ReportCell::Str(intent.to_string())),
                ("funnelStage".to_string(), ReportCell::Str(stage.to_string())),
            ]
            .into_iter()
            .collect(),
        )
    }

    #[test]
    fn topic_synthesis_groups_variants_and_emits_supported_candidates() {
        let schema = vec![
            FieldSpec::new("primaryTopic", FieldType::Str),
            FieldSpec::new("topicCluster", FieldType::Str),
            FieldSpec::new("searchIntent", FieldType::Enum(vec!["informational".into()])),
            FieldSpec::new(
                "funnelStage",
                FieldType::Enum(vec!["TOFU".into(), "MOFU".into(), "BOFU".into()]),
            ),
        ];
        let mut report = AiReportModel::new("topics", "Topics", SiteMeta::default(), schema, "topic-map");
        report.rows = vec![
            row("/a", "Personal loans", "Consumer Credit", "informational", "TOFU"),
            row("/b", "personal loans", "consumer credit", "informational", "MOFU"),
            row("/c", "Repayment", "Consumer credit", "informational", "TOFU"),
        ];

        let analysis = build_topic_analysis(&mut report, "en");
        assert_eq!(analysis["clusters"].as_array().unwrap().len(), 1);
        assert_eq!(
            analysis["cannibalizationCandidates"][0]["urls"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            analysis["internalCoverageGaps"][0]["missingFunnelStages"],
            json!(["BOFU"])
        );
        assert!(analysis["methodology"].as_str().unwrap().contains("internal"));
    }
}
