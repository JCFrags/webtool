//! Evidence for one HTTP-to-Lightpanda attempt, not a second content extractor.
use scraper::{ElementRef, Html, Selector};
use webtool_protocol::Content;
use crate::readers::Parsed;

pub const VERSION: &str = "http-lightpanda-recovery/1";

#[derive(Default)]
pub struct Evidence {
    pub blocked: Option<&'static str>,
    pub shell: Option<&'static str>,
}

fn selector(css: &str) -> Selector { Selector::parse(css).expect("constant selector") }
fn normalized(text: &str) -> String { text.split_whitespace().collect::<Vec<_>>().join(" ") }
fn visible(element: ElementRef<'_>) -> String {
    normalized(&element.descendants().filter_map(|node| {
        let value = node.value().as_text()?;
        let hidden = node.ancestors().filter_map(ElementRef::wrap).any(|ancestor| {
            matches!(ancestor.value().name(), "script"|"style"|"noscript"|"template"|"nav"|"header"|"footer")
                || ancestor.value().attr("hidden").is_some()
                || ancestor.value().attr("aria-hidden") == Some("true")
        });
        (!hidden).then_some(value.as_ref())
    }).collect::<Vec<_>>().join(" "))
}
fn loading(text: &str) -> bool {
    let text = text.trim().trim_end_matches(['.', '…', '!']).to_ascii_lowercase();
    matches!(text.as_str(), "loading"|"loading content"|"loading page"|"please wait"|"initializing")
}
fn gate(text: &str) -> Option<&'static str> {
    let text = text.trim().trim_end_matches(['.', '!', '…']).to_ascii_lowercase();
    for (label, reason) in [
        ("access denied", "access-denied page"), ("forbidden", "access-denied page"),
        ("too many requests", "rate-limit page"), ("rate limit exceeded", "rate-limit page"),
        ("verify you are human", "human-verification challenge"),
        ("verify that you are human", "human-verification challenge"),
        ("checking your browser", "browser-verification challenge"),
        ("just a moment", "browser-verification challenge"),
        ("security check", "security challenge"), ("captcha", "CAPTCHA challenge"),
        ("sign in", "login page"), ("log in", "login page"), ("login", "login page"),
        ("sign in to your account", "login page"), ("log in to your account", "login page"),
        ("authentication required", "login page"),
    ] {
        if text == label || [" |", ":", " -", " —"].iter().any(|separator| text.starts_with(&format!("{label}{separator}"))) {
            return Some(reason);
        }
    }
    None
}

pub fn inspect(bytes: &[u8]) -> Evidence {
    let Ok(source) = std::str::from_utf8(bytes) else { return Evidence::default(); };
    let dom = Html::parse_document(source);
    let mut evidence = Evidence::default();
    // A login link or an article discussing CAPTCHAs does not establish a gate.
    for heading in dom.select(&selector("title,h1")) {
        if let Some(reason) = gate(&normalized(&heading.text().collect::<String>())) {
            evidence.blocked = Some(reason);
            return evidence;
        }
    }
    let main_text = dom.select(&selector("main,article,[role=main]"))
        .map(visible).collect::<Vec<_>>().join(" ");
    let body_text = dom.select(&selector("body")).next().map(visible).unwrap_or_default();
    let has_password = dom.select(&selector("input[type=password]")).next().is_some();
    if has_password && main_text.chars().count() < 600 && body_text.chars().count() < 600 {
        evidence.blocked = Some("login form without readable main content");
        return evidence;
    }
    let has_challenge = dom.select(&selector("iframe[src*=captcha],form[action*=challenge],input[name=captcha]")).next().is_some();
    if has_challenge && main_text.chars().count() < 200 && body_text.chars().count() < 600 {
        evidence.blocked = Some("human-verification challenge without readable main content");
        return evidence;
    }
    let has_scripts = dom.select(&selector("script[src],script:not([src])")).any(|script| {
        let kind = script.value().attr("type").unwrap_or("");
        matches!(kind, ""|"module"|"text/javascript"|"application/javascript")
    });
    let empty_app = dom.select(&selector("#root,#app,#__next,#__nuxt,[data-reactroot],[role=main]"))
        .any(|element| { let text = visible(element); text.is_empty() || loading(&text) });
    let loading_status = dom.select(&selector("[role=status],[aria-busy=true]"))
        .any(|element| { let text = visible(element); text.is_empty() || loading(&text) });
    let script_required = dom.select(&selector("noscript")).any(|element| {
        let text = normalized(&element.text().collect::<String>()).to_ascii_lowercase();
        text.contains("enable javascript") || text.contains("javascript is required")
            || text.contains("javascript to run")
    });
    if empty_app && (main_text.trim().is_empty() || loading(&main_text)) && (has_scripts || script_required || loading_status) {
        evidence.shell = Some("empty application container with script/loading signals");
    } else if has_scripts && loading_status && (main_text.is_empty() || loading(&main_text)) {
        evidence.shell = Some("main content is still a loading placeholder");
    } else if has_scripts && loading(&body_text) {
        evidence.shell = Some("page contains only a scripted loading placeholder");
    }
    evidence
}

/// A heading, image, link inventory, or loading message alone is not article content.
/// No length/confidence threshold is imposed on actual prose, code, or table values.
pub fn usable(parsed: &Parsed) -> bool {
    parsed.blocks.iter().any(|block| match &block.content {
        Content::Heading { .. } | Content::Image { .. } => false,
        Content::Table { rows } => !rows.is_empty(),
        content => {
            let text = content.text();
            !text.trim().is_empty() && !loading(&text) && gate(&text).is_none()
        }
    })
}
