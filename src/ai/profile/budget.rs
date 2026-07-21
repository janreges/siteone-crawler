// SiteOne Crawler - AI profile: context-window-scaled input budgets
// (c) Jan Reges <jan.reges@siteone.cz>
//
// All character/byte budgets are calibrated at a REFERENCE context window of 128K tokens and
// derived at runtime from the user-declared --ai-context-window. Small local models therefore get
// proportionally smaller prompts (and a smaller reserved completion size) instead of overflowing.

/// Reference context window (tokens) the base budgets below are calibrated for.
const REFERENCE_CTX: f64 = 128_000.0;
/// Conservative bytes-per-token for mixed Czech/English text (Czech runs ~2.5-3 bytes/token).
const BYTES_PER_TOKEN: f64 = 2.5;

#[derive(Debug, Clone, Copy)]
pub struct ContextBudget {
    ctx_tokens: f64,
    out_tokens: u32,
    scale: f64,
    hard_cap_bytes: usize,
}

impl ContextBudget {
    /// `context_window_tokens` = --ai-context-window; `max_tokens` = --ai-max-tokens.
    pub fn new(context_window_tokens: i64, max_tokens: i64) -> Self {
        let ctx = (context_window_tokens.max(1)) as f64;
        // Reserve output tokens: at most --ai-max-tokens, at least 2048, and never more than a
        // quarter of the context. Tiny-context models are thus never asked for huge completions.
        let out = max_tokens
            .max(1)
            .min((ctx / 4.0) as i64)
            .max(2_048)
            .min(max_tokens.max(1)) as u32;
        let scale = (ctx / REFERENCE_CTX).clamp(0.05, 8.0);
        // Bytes left for the prompt after reserving the completion and a 1000-token safety margin.
        let hard_cap_bytes = (((ctx - out as f64 - 1_000.0).max(2_000.0)) * BYTES_PER_TOKEN) as usize;
        Self {
            ctx_tokens: ctx,
            out_tokens: out,
            scale,
            hard_cap_bytes,
        }
    }

    /// A base KB budget scaled by the context ratio, floored, then capped by the hard prompt cap.
    /// Returns BYTES.
    pub fn scaled(&self, base_kb: usize, floor_kb: usize) -> usize {
        let scaled_kb = (base_kb as f64 * self.scale).round() as usize;
        let with_floor = scaled_kb.max(floor_kb);
        (with_floor * 1024).min(self.hard_cap_bytes)
    }

    pub fn out_tokens(&self) -> u32 {
        self.out_tokens
    }

    pub fn context_tokens(&self) -> i64 {
        self.ctx_tokens as i64
    }

    pub fn site_summary_input(&self) -> usize {
        self.scaled(150, 16)
    }
    pub fn describe_input(&self) -> usize {
        self.scaled(12, 3)
    }
    pub fn chapter_material(&self) -> usize {
        self.scaled(200, 16)
    }
    pub fn correction_input(&self) -> usize {
        self.scaled(60, 8)
    }
    pub fn exec_input(&self) -> usize {
        self.scaled(120, 16)
    }

    /// Scale a per-chapter page cap by the context ratio, floored.
    pub fn page_cap(&self, base: usize, floor: usize) -> usize {
        ((base as f64 * self.scale).round() as usize).max(floor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_context_gives_base_budgets() {
        let b = ContextBudget::new(128_000, 32_000);
        // At the reference window, scale == 1.0, so budgets equal the base KB (in bytes), unless
        // the hard cap bites. 150 KB fits under the ~317 KB hard cap.
        assert_eq!(b.site_summary_input(), 150 * 1024);
        assert_eq!(b.chapter_material(), 200 * 1024);
        assert_eq!(b.describe_input(), 12 * 1024);
    }

    #[test]
    fn tiny_context_shrinks_budgets_but_respects_floor() {
        let b = ContextBudget::new(30_000, 32_000);
        // scale ~= 0.234 → 150*0.234 ~= 35 KB site summary, well above the 16 KB floor.
        let ss = b.site_summary_input();
        assert!(ss < 60 * 1024 && ss >= 16 * 1024, "got {ss}");
        // out tokens clamped down to <= ctx/4.
        assert!(b.out_tokens() <= 7_500);
        // describe floor (3 KB): 12*0.234 ~= 2.8 KB → floored to 3 KB.
        assert_eq!(b.describe_input(), 3 * 1024);
    }

    #[test]
    fn hard_cap_bounds_large_base_on_small_context() {
        // Even a big base cannot exceed the prompt hard cap for a small window.
        let b = ContextBudget::new(30_000, 4_000);
        assert!(b.chapter_material() <= b.scaled(100_000, 1));
    }

    #[test]
    fn large_context_scales_up_and_page_cap_grows() {
        let b = ContextBudget::new(400_000, 32_000);
        assert!(b.chapter_material() > 200 * 1024);
        assert!(b.page_cap(20, 3) >= 20);
    }

    #[test]
    fn out_tokens_never_exceeds_max_tokens() {
        let b = ContextBudget::new(1_000_000, 8_000);
        assert_eq!(b.out_tokens(), 8_000);
    }
}
