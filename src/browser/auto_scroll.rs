// SiteOne Crawler - Auto-scroll before capture
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Compiled only with the `browser` Cargo feature. Scrolls a rendered page to the bottom in steps
// so lazy-loaded images and scroll-triggered content (e.g. IntersectionObserver reveals) are
// rendered, settles the animations the scrolling started and returns to the top — before the
// HTML is captured and before screenshots (--browser-auto-scroll, on by default).

use std::time::Duration;

use chromiumoxide::Page;

use crate::browser::screenshot;

/// Each step scrolls this fraction of the viewport height, so consecutive views overlap.
const STEP_RATIO: f64 = 0.8;
/// Pause after each step, so lazy loaders and scroll observers can react.
const STEP_DELAY_MS: u64 = 120;
/// Longest time spent scrolling down (an infinite-scroll page never reaches the bottom).
const MAX_SCROLL_MS: u64 = 5000;
/// Slack between the in-page scroll cap and the timeout of the scroll phase: the last step may
/// overrun the cap, and the result still has to come back over CDP.
const SCROLL_SLACK_MS: u64 = 500;
/// Upper bound for the finish steps after scrolling (settle, back to the top, drop the injected
/// style). They always run, so this part of the render budget is kept for them.
const FINISH_TIMEOUT: Duration = Duration::from_secs(2);
/// Part of `FINISH_TIMEOUT` for the cleanup (back to the top, drop the injected style), which runs
/// even when settling ran out of time.
const CLEANUP_TIMEOUT: Duration = Duration::from_millis(800);
/// Pause after returning to the top, so the first screen repaints before the capture.
const TOP_SETTLE_MS: u64 = 300;

/// Scrolls down step by step until the bottom, a step that does not move, or the time cap.
/// Arguments: step ratio, step delay (ms) and time cap (ms). Returns the number of steps taken —
/// 0 when the document is not taller than the viewport. Never throws.
const SCROLL_DOWN_JS: &str = r#"(async function(stepRatio, delayMs, maxMs){
  var steps=0;
  try{
    var root=document.scrollingElement||document.documentElement;
    var viewport=window.innerHeight||0;
    if(!root||viewport<=0||root.scrollHeight<=viewport){return 0;}
    var step=Math.max(100,Math.floor(viewport*stepRatio));
    var deadline=Date.now()+maxMs;
    while(Date.now()<deadline){
      var before=window.scrollY;
      window.scrollTo({top:before+step,left:0,behavior:'instant'});
      steps++;
      await new Promise(function(resolve){setTimeout(resolve,delayMs);});
      if(window.scrollY<=before||window.scrollY+viewport>=root.scrollHeight-1){break;}
    }
  }catch(e){}
  return steps;
})"#;

/// Back to the top without smooth scrolling.
const SCROLL_TOP_JS: &str = "window.scrollTo({top:0,left:0,behavior:'instant'})";

/// Back to the top (again, in case settling did not get there) and removal of the style the settle
/// step injected, so it is not part of the captured HTML.
const CLEANUP_JS: &str = "window.scrollTo({top:0,left:0,behavior:'instant'});\
document.querySelectorAll('style[data-siteone-freeze]').forEach(function(s){s.remove();})";

/// In-page time cap for scrolling down with `budget` left of --browser-timeout: at most
/// `MAX_SCROLL_MS`, always leaving `SCROLL_SLACK_MS` and `FINISH_TIMEOUT` for the rest. `None` when
/// the budget is too small to scroll.
fn scroll_ms(budget: Duration) -> Option<u64> {
    let reserve = SCROLL_SLACK_MS + FINISH_TIMEOUT.as_millis() as u64;
    let available = u64::try_from(budget.as_millis())
        .unwrap_or(u64::MAX)
        .saturating_sub(reserve);
    (available >= STEP_DELAY_MS).then_some(available.min(MAX_SCROLL_MS))
}

/// The scroll-down call with its arguments.
fn scroll_down_script(max_ms: u64) -> String {
    format!("{}({}, {}, {})", SCROLL_DOWN_JS, STEP_RATIO, STEP_DELAY_MS, max_ms)
}

/// Scroll the page to the bottom and back to the top within `budget` (as far as a page blocking
/// its main thread allows). Only scrolling down is bounded by what the finish steps leave of the
/// budget; the finish steps always run afterwards, under their own timeouts, so the page never
/// stays scrolled down or keeps the injected style. Fail-soft: CDP errors are ignored.
pub async fn run(page: &Page, budget: Duration) {
    let Some(max_ms) = scroll_ms(budget) else {
        return;
    };
    let steps = tokio::time::timeout(
        budget.saturating_sub(FINISH_TIMEOUT),
        page.evaluate(scroll_down_script(max_ms)),
    )
    .await
    .ok()
    .and_then(|result| result.ok())
    .and_then(|result| result.into_value::<u64>().ok());
    // Not taller than the viewport: nothing was scrolled, nothing to finish.
    if steps == Some(0) {
        return;
    }
    // Settle the animations the scrolling started (the same step as before a screenshot) and
    // return to the top while transitions are still disabled...
    let _ = tokio::time::timeout(FINISH_TIMEOUT.saturating_sub(CLEANUP_TIMEOUT), async {
        screenshot::freeze_animations(page).await;
        tokio::time::sleep(Duration::from_millis(screenshot::FREEZE_SETTLE_MS)).await;
        let _ = page.evaluate(SCROLL_TOP_JS).await;
        tokio::time::sleep(Duration::from_millis(TOP_SETTLE_MS)).await;
    })
    .await;
    // ...then, even when settling ran out of time or scrolling failed, make sure the page is at
    // the top and drop the injected style. CDP runs the page's evaluations in order, so this comes
    // after any settle step still pending in the browser.
    let _ = tokio::time::timeout(CLEANUP_TIMEOUT, page.evaluate(CLEANUP_JS)).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrolling_is_capped_and_leaves_room_to_finish_within_the_budget() {
        assert_eq!(scroll_ms(Duration::from_secs(30)), Some(MAX_SCROLL_MS));
        assert_eq!(scroll_ms(Duration::from_millis(4000)), Some(1500));
        assert_eq!(scroll_ms(Duration::from_millis(2620)), Some(STEP_DELAY_MS));
        assert_eq!(scroll_ms(Duration::from_millis(2600)), None);
        assert_eq!(scroll_ms(Duration::ZERO), None);
        // The in-page cap, its slack and the finish steps together stay within the budget.
        for budget_ms in [2620, 3000, 5000, 7500, 30_000] {
            let budget = Duration::from_millis(budget_ms);
            let scroll = scroll_ms(budget).expect("room to scroll");
            assert!(
                Duration::from_millis(scroll + SCROLL_SLACK_MS) + FINISH_TIMEOUT <= budget,
                "{budget_ms} ms"
            );
        }
    }

    #[test]
    fn scroll_script_gets_the_step_ratio_delay_and_cap() {
        let script = scroll_down_script(4000);
        assert!(
            script.starts_with("(async function(stepRatio, delayMs, maxMs){"),
            "{script}"
        );
        assert!(script.ends_with("})(0.8, 120, 4000)"), "{script}");
    }
}
