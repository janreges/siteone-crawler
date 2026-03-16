// SiteOne Crawler - ManagerStats
// (c) Jan Reges <jan.reges@siteone.cz>

use std::collections::HashMap;
use std::time::Instant;

use crate::components::super_table::SuperTable;
use crate::components::super_table_column::SuperTableColumn;
use crate::utils;

#[derive(Debug, Default)]
pub struct ManagerStats {
    /// Total exec times of analyzer methods
    exec_times: HashMap<String, f64>,

    /// Total exec counts of analyzer methods
    exec_counts: HashMap<String, usize>,
}

impl ManagerStats {
    pub fn new() -> Self {
        Self {
            exec_times: HashMap::new(),
            exec_counts: HashMap::new(),
        }
    }

    /// Measure and increment exec time and count of analyzer method
    pub fn measure_exec_time(&mut self, class: &str, method: &str, start_time: Instant) {
        let elapsed = start_time.elapsed().as_secs_f64();
        let key = format!("{}::{}", class, method);

        *self.exec_times.entry(key.clone()).or_insert(0.0) += elapsed;
        *self.exec_counts.entry(key).or_insert(0) += 1;
    }

    pub fn get_super_table(
        &self,
        apl_code: &str,
        title: &str,
        empty_table_message: &str,
        external_times: Option<&HashMap<String, f64>>,
        external_counts: Option<&HashMap<String, usize>>,
    ) -> SuperTable {
        let mut data: Vec<HashMap<String, String>> = Vec::new();

        // Internal stats
        for (class_and_method, exec_time) in &self.exec_times {
            let short_name = class_and_method
                .rsplit("::")
                .next()
                .map(|method| {
                    let class_part = class_and_method.split("::").next().unwrap_or(class_and_method);
                    let short_class = class_part.rsplit('/').next().unwrap_or(class_part);
                    let short_class = short_class.rsplit('\\').next().unwrap_or(short_class);
                    format!("{}::{}", short_class, method)
                })
                .unwrap_or_else(|| class_and_method.clone());

            let mut row = HashMap::new();
            row.insert("classAndMethod".to_string(), short_name);
            row.insert("execTime".to_string(), format!("{}", exec_time));
            row.insert(
                "execTimeFormatted".to_string(),
                utils::get_formatted_duration(*exec_time),
            );
            row.insert(
                "execCount".to_string(),
                format!("{}", self.exec_counts.get(class_and_method).copied().unwrap_or(0)),
            );
            data.push(row);
        }

        // External stats (if any)
        if let Some(ext_times) = external_times {
            for (class_and_method, exec_time) in ext_times {
                let short_name = class_and_method
                    .rsplit("::")
                    .next()
                    .map(|method| {
                        let class_part = class_and_method.split("::").next().unwrap_or(class_and_method);
                        let short_class = class_part.rsplit('/').next().unwrap_or(class_part);
                        let short_class = short_class.rsplit('\\').next().unwrap_or(short_class);
                        format!("{}::{}", short_class, method)
                    })
                    .unwrap_or_else(|| class_and_method.clone());

                let mut row = HashMap::new();
                row.insert("classAndMethod".to_string(), short_name);
                row.insert("execTime".to_string(), format!("{}", exec_time));
                row.insert(
                    "execTimeFormatted".to_string(),
                    utils::get_formatted_duration(*exec_time),
                );
                row.insert(
                    "execCount".to_string(),
                    format!(
                        "{}",
                        external_counts
                            .and_then(|c| c.get(class_and_method))
                            .copied()
                            .unwrap_or(0)
                    ),
                );
                data.push(row);
            }
        }

        let columns = vec![
            SuperTableColumn::new(
                "classAndMethod".to_string(),
                "Class::method".to_string(),
                -1, // AUTO_WIDTH
                None,
                None,
                false,
                false,
                false,
                true,
                None,
            ),
            SuperTableColumn::new(
                "execTime".to_string(),
                "Exec time".to_string(),
                9,
                Some(Box::new(|value: &str, _render_into: &str| {
                    if let Ok(v) = value.parse::<f64>() {
                        utils::get_colored_request_time(v, 9)
                    } else {
                        value.to_string()
                    }
                })),
                None,
                false,
                false,
                false,
                true,
                None,
            ),
            SuperTableColumn::new(
                "execCount".to_string(),
                "Exec count".to_string(),
                -1, // AUTO_WIDTH
                None,
                None,
                false,
                false,
                false,
                true,
                None,
            ),
        ];

        let mut super_table = SuperTable::new(
            apl_code.to_string(),
            title.to_string(),
            empty_table_message.to_string(),
            columns,
            false,
            Some("execTime".to_string()),
            "DESC".to_string(),
            None,
            None,
            None,
        );

        super_table.set_data(data);
        super_table
    }

    pub fn get_exec_times(&self) -> &HashMap<String, f64> {
        &self.exec_times
    }

    pub fn get_exec_counts(&self) -> &HashMap<String, usize> {
        &self.exec_counts
    }
}
