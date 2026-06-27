// SiteOne Crawler - Cookie consent dismissal
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Compiled only with the `browser` Cargo feature. Best-effort, fail-soft removal of
// cookie consent banners before a screenshot is taken: clicks known "reject/accept"
// controls across major CMPs (incl. shadow DOM), removes scroll-lock, then hides a
// curated set of consent containers plus any fixed/sticky high-z-index overlay whose
// text matches cookie/consent keywords (English + Czech). Also honours a user-supplied
// list of CSS selectors (--screenshot-hide-selector).

use chromiumoxide::Page;

use crate::options::core_options::CoreOptions;

/// The CMP-dismissal + curated-hide + heuristic logic (no IIFE wrapper; that is added by
/// `build_script`). Pure JS that must never throw (everything is wrapped in try/catch).
const FULL_JS: &str = r#"
var __reject__=['#onetrust-reject-all-handler','#CybotCookiebotDialogBodyButtonDecline','.didomi-continue-without-agreeing','[data-testid="uc-deny-all-button"]','.qc-cmp2-summary-buttons button[mode="secondary"]','#truste-consent-required','.cmplz-deny','.cc-deny','.cc-dismiss','#cookie-reject','.cookie-reject','.cookies-reject','button[aria-label="Reject all"]','button[aria-label="Decline all"]'];
var __accept__=['#onetrust-accept-btn-handler','#CybotCookiebotDialogBodyLevelButtonLevelOptinAllowAll','#CybotCookiebotDialogBodyButtonAccept','[data-testid="uc-accept-all-button"]','.qc-cmp2-summary-buttons button[mode="primary"]','#truste-consent-button','.cmplz-accept','.cc-allow','.cc-accept','.cc-accept-all','#cookie-accept','.cookie-accept','.cookies-accept','button[aria-label="Accept all"]','button[aria-label="Accept all cookies"]'];
function __click__(root,sels){for(var i=0;i<sels.length;i++){try{var e=root.querySelectorAll(sels[i]);for(var j=0;j<e.length;j++){try{e[j].click();}catch(x){}}}catch(x){}}}
__click__(document,__reject__);
try{document.querySelectorAll('*').forEach(function(el){if(el.shadowRoot){__click__(el.shadowRoot,__reject__);__click__(el.shadowRoot,__accept__);}});}catch(x){}
__click__(document,__accept__);
var __lock__=['didomi-popup-open','qc-cmp-ui-showing','sp-message-open','cmplz-blocked','modal-open','no-scroll','noscroll','overflow-hidden','cookie-open'];
[document.documentElement,document.body].forEach(function(el){if(!el)return;__lock__.forEach(function(c){try{el.classList.remove(c);}catch(x){}});try{el.style.overflow='';el.style.position='';}catch(x){}});
var __hide__=['#onetrust-consent-sdk','#onetrust-banner-sdk','#CookieConsent','#CybotCookiebotDialog','#cookiescript_injected','#didomi-host','#didomi-popup','#usercentrics-root','#usercentrics-cmp-ui','#qc-cmp2-container','.qc-cmp2-container','#truste-consent-track','.truste_overlay','.truste_box_overlay','[id^="sp_message_container"]','#cmplz-cookiebanner-container','.cmplz-cookiebanner','#cookie-law-info-bar','#cookie-notice','.cookie-notice','.cookie-banner','#cookie-banner','.cookie-consent','#cookie-consent','#cookieConsent','.cookieconsent','.cc-window','.cookie-bar','#cookiebar','.cookie-popup','.cookie-modal','.cookies-popup','.gdpr','.gdpr-banner','#gdpr','.iubenda-cs-container','#iubenda-cs-banner','.osano-cm-window','#termly-code-snippet-support','#cookiebanner','.cookiebanner','[aria-label="cookieconsent"]','[class*="cookie"][class*="consent"]','[id*="cookie"][class*="banner"]'];
__hide__.forEach(function(sel){try{document.querySelectorAll(sel).forEach(function(el){el.style.setProperty('display','none','important');});}catch(x){}});
try{var __kw__=/cookie|consent|gdpr|souhlas|p[řr]ijmout|odm[íi]tnout|soukrom|z[áa]sady ochrany|personaliz/i;var __all__=document.querySelectorAll('body *');for(var i=0;i<__all__.length;i++){var el=__all__[i];var st=getComputedStyle(el);if((st.position==='fixed'||st.position==='sticky')&&((parseInt(st.zIndex,10)||0)>=1000)){var t=(el.innerText||'').slice(0,400);if(__kw__.test(t)){el.style.setProperty('display','none','important');}}}}catch(x){}
"#;

/// Build the injectable script. Always hides the user-supplied `selectors`; when `full`
/// is true, also runs the CMP dismissal + curated hide + heuristic logic.
fn build_script(selectors: &[String], full: bool) -> String {
    let user_json = serde_json::to_string(selectors).unwrap_or_else(|_| "[]".to_string());
    let mut s = String::new();
    s.push_str("(function(){try{var __user__=");
    s.push_str(&user_json);
    s.push(';');
    if full {
        s.push_str(FULL_JS);
    }
    s.push_str("try{__user__.forEach(function(sel){try{document.querySelectorAll(sel).forEach(function(el){el.style.setProperty('display','none','important');});}catch(e){}});}catch(e){}");
    s.push_str("}catch(e){}})();");
    s
}

/// Inject and run the dismissal/hide script in the page. Fail-soft: returns an error
/// string only on a CDP evaluation failure; callers ignore it.
pub async fn dismiss(page: &Page, options: &CoreOptions) -> Result<(), String> {
    let selectors: Vec<String> = options
        .screenshot_hide_selector
        .as_deref()
        .map(|s| {
            s.split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let script = build_script(&selectors, options.screenshot_hide_cookie_banners);
    page.evaluate(script).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_script_includes_user_selectors_and_respects_full_flag() {
        let full = build_script(&["#my-banner".to_string(), ".foo".to_string()], true);
        assert!(full.contains("#my-banner"));
        assert!(full.contains(".foo"));
        assert!(full.contains("onetrust")); // full CMP logic present

        let minimal = build_script(&[".only".to_string()], false);
        assert!(minimal.contains(".only"));
        assert!(!minimal.contains("onetrust")); // CMP logic omitted when not full
    }
}
