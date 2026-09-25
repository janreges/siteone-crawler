// SiteOne Crawler - paged sections of the HTML report
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Reports of large crawls used to put every visited URL and every gallery image into the DOM, which
// froze the browser (#99). Sections with more than PAGED_THRESHOLD items therefore embed their rows
// as compact JSON, server-render only the first page (the no-JS fallback) and leave paging, sorting
// and filtering to the small vanilla-JS pager in template.html. Both sides render rows with the same
// templates: `VisitedUrlRow::to_html`, `GalleryImage::to_html` and `badge_html` here mirror
// `pagedVisitedUrlRow`, `pagedGalleryItem` and `pagedBadge` in template.html - change them together.

use serde_json::{Value, json};

use super::report::html_escape;

/// Sections with more items than this are paged; smaller ones keep the classic full rendering.
pub const PAGED_THRESHOLD: usize = 1000;

/// Items rendered server-side (the first page) and the pager's default page size.
pub const FIRST_PAGE_SIZE: usize = 100;

/// Thumbnail size of the gallery's default "small" mode, in pixels.
pub const DEFAULT_THUMBNAIL_PX: u32 = 140;

/// Element id prefix of the paged Visited URLs section.
pub const VISITED_URLS_ID: &str = "visited-urls";

/// Element id prefix of the paged Image Gallery section.
pub const IMAGE_GALLERY_ID: &str = "image-gallery";

/// Visited URLs columns: header label, index of the sort value in the JSON row, sort type.
const VISITED_URLS_COLUMNS: [(&str, usize, &str); 6] = [
    ("URL", 0, "string"),
    ("Status", 2, "number"),
    ("Type", 5, "string"),
    ("Time (s)", 6, "number"),
    ("Size", 9, "number"),
    ("Cache", 12, "number"),
];

/// Badge tone letter ("" = plain text) of a `utils::get_color_text` colour name. Magenta is shown as
/// orange, like everywhere else in the HTML report.
pub fn tone_of_color(color: &str) -> &'static str {
    match color {
        "green" => "g",
        "yellow" => "y",
        "magenta" => "o",
        "red" => "r",
        _ => "",
    }
}

/// A cell value, as a coloured badge when it has a tone (`pagedBadge` in template.html).
pub fn badge_html(text: &str, tone: &str) -> String {
    let class = match tone {
        "g" => "green",
        "y" => "yellow",
        "o" => "orange",
        "r" => "red",
        _ => return html_escape(text),
    };
    format!("<span class=\"badge {}\">{}</span>", class, html_escape(text))
}

/// Compact JSON that is safe inside `<script type="application/json">`: `<`, `>` and `&` are written
/// as `\u003c`, `\u003e` and `\u0026`, so the payload can neither close the script element nor be
/// touched by the report's HTML post-processing (which only rewrites real tags).
pub fn json_for_script(value: &Value) -> String {
    let json = value.to_string();
    let mut out = String::with_capacity(json.len() + json.len() / 50);
    for ch in json.chars() {
        match ch {
            '<' => out.push_str("\\u003c"),
            '>' => out.push_str("\\u003e"),
            '&' => out.push_str("\\u0026"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            _ => out.push(ch),
        }
    }
    out
}

/// One Visited URLs row, already formatted. `to_json` gives the pager's array row
/// `[url, urlText, status, statusText, statusTone, type, time, timeText, timeTone, size, sizeText,
/// sizeTone, cacheSort, cacheText, cacheTone]`.
#[derive(Debug, Clone, PartialEq)]
pub struct VisitedUrlRow {
    pub url: String,
    pub url_text: String,
    pub status: i32,
    pub status_text: String,
    pub status_tone: &'static str,
    pub type_name: &'static str,
    pub time: f64,
    pub time_text: String,
    pub time_tone: &'static str,
    pub size: i64,
    pub size_text: String,
    pub size_tone: &'static str,
    pub cache_sort: f64,
    pub cache_text: String,
    pub cache_tone: &'static str,
}

impl VisitedUrlRow {
    pub fn to_json(&self) -> Value {
        json!([
            self.url,
            self.url_text,
            self.status,
            self.status_text,
            self.status_tone,
            self.type_name,
            self.time,
            self.time_text,
            self.time_tone,
            self.size,
            self.size_text,
            self.size_tone,
            self.cache_sort,
            self.cache_text,
            self.cache_tone
        ])
    }

    /// Mirrors `pagedVisitedUrlRow` in template.html.
    pub fn to_html(&self) -> String {
        format!(
            "<tr><td class=\"url\"><a href=\"{}\" target=\"_blank\">{}</a></td><td class=\"status\">{}</td><td class=\"type\">{}</td><td class=\"time\">{}</td><td class=\"size\">{}</td><td class=\"cacheLifetime\">{}</td></tr>",
            html_escape(&self.url),
            html_escape(&self.url_text),
            badge_html(&self.status_text, self.status_tone),
            html_escape(self.type_name),
            badge_html(&self.time_text, self.time_tone),
            badge_html(&self.size_text, self.size_tone),
            badge_html(&self.cache_text, self.cache_tone),
        )
    }
}

/// One Image Gallery item. `to_json` gives the pager's array item `[url, size, type, source, description]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GalleryImage {
    pub url: String,
    pub size: i64,
    pub image_type: String,
    pub source: String,
    pub description: String,
}

impl GalleryImage {
    pub fn to_json(&self) -> Value {
        json!([self.url, self.size, self.image_type, self.source, self.description])
    }

    /// Mirrors `pagedGalleryItem` in template.html.
    pub fn to_html(&self, thumbnail_px: u32) -> String {
        format!(
            "<a href=\"{url}\" target=\"_blank\"><img loading=\"lazy\" width=\"{px}\" height=\"{px}\" src=\"{url}\" alt=\"{desc}\" title=\"{desc}\"></a>",
            url = html_escape(&self.url),
            px = thumbnail_px,
            desc = html_escape(&self.description),
        )
    }
}

/// Paged Visited URLs section: controls, table with the first page, pager, JSON rows.
pub fn visited_urls_section_html(rows: &[VisitedUrlRow]) -> String {
    let id = VISITED_URLS_ID;
    let total = rows.len();
    let mut html = section_start(id, "visited-urls", total);
    html.push_str(&format!(
        "<div class=\"paged-controls\"><input type=\"text\" class=\"fulltext paged-fulltext\" data-paged-id=\"{id}\" style=\"width: 300px;\" placeholder=\"Fulltext search\" aria-label=\"Fulltext search\"><span id=\"foundRows_{id}\" class=\"found-rows\">Found {total} row(s).</span>{}{}</div>",
        page_size_select(id),
        pager_buttons(id, total),
    ));
    html.push_str(&no_js_note(total, "rows"));
    html.push_str(&format!(
        "<div class=\"table-container\"><table id=\"{id}\" border=\"1\" class=\"table table-bordered table-hover visited-urls\" style=\"border-collapse: collapse;\"><thead><tr>"
    ));
    for (label, sort_index, data_type) in VISITED_URLS_COLUMNS {
        html.push_str(&format!(
            "<th class=\"paged-th\" data-paged-id=\"{id}\" data-sort=\"{sort_index}\" data-type=\"{data_type}\" data-label=\"{label}\">{label}</th>"
        ));
    }
    html.push_str(&format!("</tr></thead><tbody id=\"{id}_items\">"));
    for row in rows.iter().take(FIRST_PAGE_SIZE) {
        html.push_str(&row.to_html());
    }
    html.push_str("</tbody></table></div>");
    html.push_str(&format!(
        "<div class=\"paged-controls\">{}</div>",
        pager_buttons(id, total)
    ));
    html.push_str(&data_script(id, rows.iter().map(VisitedUrlRow::to_json)));
    html.push_str("</div>");
    html
}

/// Paged Image Gallery section: pager, gallery with the first page, pager, JSON items. The
/// type/source/size filters of the gallery form are built by the pager from the JSON.
pub fn image_gallery_section_html(images: &[GalleryImage]) -> String {
    let id = IMAGE_GALLERY_ID;
    let total = images.len();
    let mut html = section_start(id, "image-gallery", total);
    html.push_str(&format!(
        "<div class=\"paged-controls\">{}{}</div>",
        page_size_select(id),
        pager_buttons(id, total)
    ));
    html.push_str(&no_js_note(total, "images"));
    html.push_str(&format!(
        "<div id=\"igc\" class=\"small\"><div id=\"igcf\" class=\"scaleDown\"><div id=\"{id}_items\" class=\"image-gallery\">"
    ));
    for image in images.iter().take(FIRST_PAGE_SIZE) {
        html.push_str(&image.to_html(DEFAULT_THUMBNAIL_PX));
    }
    html.push_str("</div></div></div>");
    html.push_str(&format!(
        "<div class=\"paged-controls\">{}</div>",
        pager_buttons(id, total)
    ));
    html.push_str(&data_script(id, images.iter().map(GalleryImage::to_json)));
    html.push_str("</div>");
    html
}

fn section_start(id: &str, kind: &str, total: usize) -> String {
    format!("<div class=\"paged\" data-paged-id=\"{id}\" data-paged-kind=\"{kind}\" data-total=\"{total}\">")
}

fn page_size_select(id: &str) -> String {
    format!(
        "<label>Per page <select class=\"paged-size\" data-paged-id=\"{id}\"><option value=\"100\" selected>100</option><option value=\"500\">500</option><option value=\"1000\">1000</option></select></label>"
    )
}

fn pager_buttons(id: &str, total: usize) -> String {
    let pages = total.div_ceil(FIRST_PAGE_SIZE).max(1);
    format!(
        "<button type=\"button\" class=\"btn paged-prev\" data-paged-id=\"{id}\" disabled>&laquo; Previous</button><span class=\"paged-info\" data-paged-id=\"{id}\">Page 1 of {pages} ({total} items)</span><button type=\"button\" class=\"btn paged-next\" data-paged-id=\"{id}\">Next &raquo;</button>"
    )
}

fn no_js_note(total: usize, noun: &str) -> String {
    format!(
        "<p class=\"paged-nojs\">Showing the first {} of {} {}. Open the report in a browser with JavaScript enabled to browse all of them.</p>",
        FIRST_PAGE_SIZE.min(total),
        total,
        noun
    )
}

/// The JSON array written item by item, so a crawl with 100k URLs needs no second in-memory copy.
fn data_script(id: &str, items: impl Iterator<Item = Value>) -> String {
    let mut json = String::from("[");
    for (i, item) in items.enumerate() {
        if i > 0 {
            json.push(',');
        }
        json.push_str(&json_for_script(&item));
    }
    json.push(']');
    format!("<script type=\"application/json\" id=\"{id}_data\">{json}</script>")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_row() -> VisitedUrlRow {
        VisitedUrlRow {
            url: "https://example.com/a?x=1&y=<2>".to_string(),
            url_text: "/a?x=1&y=<2>".to_string(),
            status: 404,
            status_text: "404".to_string(),
            status_tone: "o",
            type_name: "HTML",
            time: 0.25,
            time_text: "250 ms".to_string(),
            time_tone: "g",
            size: 2048,
            size_text: "2 kB".to_string(),
            size_tone: "",
            cache_sort: -1.0,
            cache_text: "0s (no-cache)".to_string(),
            cache_tone: "r",
        }
    }

    #[test]
    fn json_for_script_cannot_close_the_script_element() {
        let value = json!(["</script><script>alert(1)</script>", "a & b", "\u{2028}"]);
        let out = json_for_script(&value);
        assert!(!out.contains('<') && !out.contains('>') && !out.contains('&'), "{out}");
        assert!(!out.contains('\u{2028}'), "{out}");
        assert_eq!(
            serde_json::from_str::<Value>(&out).unwrap(),
            value,
            "still the same JSON"
        );
    }

    #[test]
    fn badges_use_the_report_badge_classes() {
        assert_eq!(badge_html("200", "g"), "<span class=\"badge green\">200</span>");
        assert_eq!(badge_html("1.5 s", "o"), "<span class=\"badge orange\">1.5 s</span>");
        assert_eq!(badge_html("<b>", ""), "&lt;b&gt;");
        assert_eq!(tone_of_color("magenta"), "o");
        assert_eq!(tone_of_color("gray"), "");
    }

    #[test]
    fn colour_helpers_keep_the_console_thresholds() {
        use crate::utils::{get_cache_lifetime_color, get_request_time_color, get_status_code_color};
        assert_eq!(get_status_code_color(200), ("green", false));
        assert_eq!(get_status_code_color(301), ("yellow", true));
        assert_eq!(get_status_code_color(404), ("magenta", true));
        assert_eq!(get_status_code_color(503), ("red", true));
        assert_eq!(get_status_code_color(-1), ("red", true));
        assert_eq!(get_request_time_color(0.2), ("green", false));
        assert_eq!(get_request_time_color(0.7), ("yellow", false));
        assert_eq!(get_request_time_color(1.5), ("magenta", true));
        assert_eq!(get_request_time_color(2.0), ("red", true));
        assert_eq!(get_cache_lifetime_color(0), "red");
        assert_eq!(get_cache_lifetime_color(300), "magenta");
        assert_eq!(get_cache_lifetime_color(86_400), "yellow");
        assert_eq!(get_cache_lifetime_color(86_401), "green");
    }

    #[test]
    fn visited_url_row_html_and_json_line_up() {
        let row = sample_row();
        assert_eq!(
            row.to_html(),
            "<tr><td class=\"url\"><a href=\"https://example.com/a?x=1&amp;y=&lt;2&gt;\" target=\"_blank\">/a?x=1&amp;y=&lt;2&gt;</a></td><td class=\"status\"><span class=\"badge orange\">404</span></td><td class=\"type\">HTML</td><td class=\"time\"><span class=\"badge green\">250 ms</span></td><td class=\"size\">2 kB</td><td class=\"cacheLifetime\"><span class=\"badge red\">0s (no-cache)</span></td></tr>"
        );
        let json = row.to_json();
        assert_eq!(json.as_array().unwrap().len(), 15);
        for (_, sort_index, _) in VISITED_URLS_COLUMNS {
            assert!(!json[sort_index].is_null());
        }
        assert_eq!(json[2], 404);
        assert_eq!(json[6], 0.25);
        assert_eq!(json[9], 2048);
        assert_eq!(json[12], -1.0);
        assert_eq!(json[4], "o");
    }

    #[test]
    fn gallery_item_html_and_json() {
        let image = GalleryImage {
            url: "https://example.com/a.png".to_string(),
            size: 512,
            image_type: "png".to_string(),
            source: "<img src>".to_string(),
            description: "512 B (image/png), found as <img src> on https://example.com/".to_string(),
        };
        assert_eq!(
            image.to_html(DEFAULT_THUMBNAIL_PX),
            "<a href=\"https://example.com/a.png\" target=\"_blank\"><img loading=\"lazy\" width=\"140\" height=\"140\" src=\"https://example.com/a.png\" alt=\"512 B (image/png), found as &lt;img src&gt; on https://example.com/\" title=\"512 B (image/png), found as &lt;img src&gt; on https://example.com/\"></a>"
        );
        assert_eq!(
            image.to_json(),
            json!([
                "https://example.com/a.png",
                512,
                "png",
                "<img src>",
                "512 B (image/png), found as <img src> on https://example.com/"
            ])
        );
    }

    #[test]
    fn sections_render_only_the_first_page() {
        let rows: Vec<VisitedUrlRow> = (0..250)
            .map(|i| VisitedUrlRow {
                url: format!("https://example.com/{i}"),
                ..sample_row()
            })
            .collect();
        let html = visited_urls_section_html(&rows);
        assert_eq!(html.matches("<tr><td class=\"url\">").count(), FIRST_PAGE_SIZE);
        assert!(html.contains("data-total=\"250\""));
        assert!(html.contains("Page 1 of 3 (250 items)"));
        assert!(html.contains("<script type=\"application/json\" id=\"visited-urls_data\">"));

        let images: Vec<GalleryImage> = (0..120)
            .map(|i| GalleryImage {
                url: format!("https://example.com/{i}.png"),
                size: 10,
                image_type: "png".to_string(),
                source: "<img src>".to_string(),
                description: "x".to_string(),
            })
            .collect();
        let html = image_gallery_section_html(&images);
        assert_eq!(html.matches("<img loading=\"lazy\"").count(), FIRST_PAGE_SIZE);
        assert!(html.contains("<div id=\"igc\" class=\"small\">"));
        assert!(html.contains("<script type=\"application/json\" id=\"image-gallery_data\">"));
    }

    #[test]
    fn template_pager_mirrors_the_rust_templates() {
        let template = include_str!("template.html");
        for fragment in [
            r#"var PAGED_TONES = {g: 'green', y: 'yellow', o: 'orange', r: 'red'};"#,
            r#"return cls ? '<span class="badge ' + cls + '">' + pagedEsc(text) + '</span>' : pagedEsc(text);"#,
            r#"return '<tr><td class="url"><a href="' + pagedEsc(r[0]) + '" target="_blank">' + pagedEsc(r[1]) + '</a></td>' +"#,
            r#"'<td class="status">' + pagedBadge(r[3], r[4]) + '</td>' +"#,
            r#"'<td class="type">' + pagedEsc(r[5]) + '</td>' +"#,
            r#"'<td class="time">' + pagedBadge(r[7], r[8]) + '</td>' +"#,
            r#"'<td class="size">' + pagedBadge(r[10], r[11]) + '</td>' +"#,
            r#"'<td class="cacheLifetime">' + pagedBadge(r[13], r[14]) + '</td></tr>';"#,
            r#"return '<a href="' + pagedEsc(r[0]) + '" target="_blank"><img loading="lazy" width="' + px + '" height="' + px +"#,
            r#"'" src="' + pagedEsc(r[0]) + '" alt="' + pagedEsc(r[4]) + '" title="' + pagedEsc(r[4]) + '"></a>';"#,
            "initPagedSections();",
        ] {
            assert!(template.contains(fragment), "template.html lost: {fragment}");
        }
    }

    #[test]
    fn template_debounces_fulltext_search_once_per_table() {
        let template = include_str!("template.html");
        assert!(
            !template.contains("debounce(tableFulltext, 250)(tableId, searchTerm)"),
            "a new debouncer per keyup never cancels the previous scan"
        );
        assert!(template.contains("tableFulltextDebouncers[tableId] = debounce(tableFulltext, 250);"));
    }
}
