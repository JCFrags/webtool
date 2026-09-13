//! Paid-result checks must run before HTML labels and redirect context are lost.
use anyhow::{anyhow, bail, Result};
use scraper::{ElementRef, Html, Selector};
use url::Url;
use webtool_protocol::SearchResult;

struct Layout {
    endpoint: &'static str,
    query_key: &'static str,
    result: &'static str,
    link: &'static str,
    title: &'static str,
    snippet: &'static str,
}

// Keep the four existing provider formats. Unknown layouts have no broad fallback.
fn layout(provider: &str) -> Result<Layout> {
    let (endpoint, query_key, result, link, title, snippet) = match provider {
        "duckduckgo" => ("https://html.duckduckgo.com/html/", "q", "div.result.web-result", "a.result__a", "a.result__a", "a.result__snippet"),
        "brave" => ("https://search.brave.com/search", "q", "div[data-type='web']", "a.l1", "div.search-snippet-title", "div.generic-snippet"),
        "startpage" => ("https://www.startpage.com/search", "q", "div.result", "a.result-title", "h2.wgl-title", "p.description"),
        "yahoo" => ("https://search.yahoo.com/search", "p", "div.algo-sr", "div.compTitle a", "div.compTitle a h3 span", "div.compText"),
        _ => bail!("unsupported search provider: {provider}"),
    };
    Ok(Layout { endpoint, query_key, result, link, title, snippet })
}

#[cfg(feature = "web-search")]
pub(super) async fn search(client: &reqwest::Client, provider: &str, query: &str, limit: usize, max_bytes: usize) -> Result<Vec<SearchResult>> {
    use std::time::Duration;
    let format = layout(provider)?;
    let mut request = client.get(format.endpoint).query(&[(format.query_key, query)]);
    if provider == "yahoo" {
        // Preserve the existing provider's language and safe-search settings.
        request = request.header("Cookie", "sB=v=1&vm=p&fl=1&vl=lang_en&pn=10");
    }
    // Preserve the pinned adapter's header deadline and client's socket ceiling.
    let mut response = tokio::time::timeout(
        Duration::from_millis(metadata_search_engine_rs::engines::DEFAULT_TIMEOUT_MS),
        request.send(),
    ).await.map_err(|_| anyhow!("provider timeout"))??.error_for_status()?;
    if response.content_length().is_some_and(|n| n > max_bytes as u64) {
        bail!("search response exceeds the {max_bytes}-byte limit");
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if body.len().saturating_add(chunk.len()) > max_bytes {
            bail!("search response exceeds the {max_bytes}-byte limit after decompression");
        }
        body.extend_from_slice(&chunk);
    }
    parse(provider, &String::from_utf8_lossy(&body), limit)
}

fn selector(value: &str) -> Result<Selector> {
    Selector::parse(value).map_err(|e| anyhow!("invalid search selector: {e:?}"))
}

fn paid_marker(value: &str) -> bool {
    // Match structural tokens, not arbitrary substrings such as "download".
    value.to_ascii_lowercase().split(|c: char| !c.is_ascii_alphanumeric())
        .any(|part| matches!(part, "ad" | "ads" | "advert" | "advertisement" | "advertising" | "sponsored" | "promoted" | "paid" | "adsbygoogle"))
}

fn paid_label(value: &str) -> bool {
    matches!(value.trim().to_ascii_lowercase().as_str(), "ad" | "ads" | "advertisement" | "sponsored" | "sponsored result" | "sponsored results" | "paid result" | "promoted")
}

fn paid_element(element: ElementRef<'_>) -> bool {
    let node = element.value();
    if ["class", "id", "data-type", "data-testid", "data-component"]
        .iter().any(|key| node.attr(key).is_some_and(paid_marker)) {
        return true;
    }
    if ["data-label", "aria-label"].iter().any(|key| node.attr(key).is_some_and(paid_label)) {
        return true;
    }
    if node.attr("rel").is_some_and(|v| v.split_ascii_whitespace().any(|part| part.eq_ignore_ascii_case("sponsored"))) {
        return true;
    }
    ["data-ad", "data-sponsored", "data-promoted"].iter().any(|key| {
        node.attr(key).is_some_and(|v| !matches!(v.to_ascii_lowercase().as_str(), "false" | "0" | "no"))
    })
}

fn paid_context(card: ElementRef<'_>, titles: &Selector, snippets: &Selector) -> bool {
    // An ad wrapper can contain an otherwise ordinary-looking result card.
    paid_element(card) || card.ancestors().filter_map(ElementRef::wrap).any(paid_element)
        || card.descendants().filter_map(ElementRef::wrap).any(|el| {
            if paid_element(el) { return true; }
            // Read standalone badges, not words in a result's title or snippet.
            matches!(el.value().name(), "span" | "label" | "small")
                && !titles.matches(&el) && !snippets.matches(&el)
                && !el.ancestors().filter_map(ElementRef::wrap).any(|parent| titles.matches(&parent) || snippets.matches(&parent))
                && paid_label(&el.text().collect::<String>())
        })
}

fn host_is(host: &str, domain: &str) -> bool {
    host == domain || host.strip_suffix(domain).is_some_and(|prefix| prefix.ends_with('.'))
}

pub(super) fn paid_url(url: &Url) -> bool {
    let host = url.host_str().unwrap_or("").trim_end_matches('.');
    if ["googleadservices.com", "googlesyndication.com", "doubleclick.net"].iter().any(|domain| host_is(host, domain)) {
        return true;
    }
    let path = url.path().to_ascii_lowercase();
    if host_is(host, "duckduckgo.com") && (path == "/y.js" || url.query_pairs().any(|(key, _)| matches!(key.as_ref(), "ad_provider" | "ad_domain" | "ad_type"))) {
        return true;
    }
    if host_is(host, "bing.com") && (path == "/aclick" || path.starts_with("/aclick/")) {
        return true;
    }
    // Explicit paid-click identifiers are rejected, not removed to disguise the URL.
    url.query_pairs().any(|(key, value)| {
        match key.to_ascii_lowercase().as_str() {
            "gclid" | "dclid" | "msclkid" | "gbraid" | "wbraid" => true,
            "utm_medium" => matches!(value.to_ascii_lowercase().as_str(), "cpc" | "ppc" | "paid" | "paidsearch" | "paid-search" | "paid_search" | "paid-social" | "paid_social"),
            _ => false,
        }
    })
}

fn destination(provider: &str, base: &Url, href: &str) -> Option<Url> {
    // Only DDG's existing organic link wrapper accepts a relative destination.
    if href.trim().is_empty() { return None; }
    let raw = Url::parse(href).ok().or_else(|| {
        let joined = base.join(href).ok()?;
        (provider == "duckduckgo" && host_is(joined.host_str()?, "duckduckgo.com") && matches!(joined.path(), "/l/" | "/l" | "/y.js")).then_some(joined)
    })?;
    if paid_url(&raw) { return None; }
    let host = raw.host_str().unwrap_or("").trim_end_matches('.');
    let decoded = if provider == "duckduckgo" && host_is(host, "duckduckgo.com") && matches!(raw.path(), "/l/" | "/l") {
        raw.query_pairs().find(|(key, _)| key == "uddg")?.1.into_owned()
    } else if provider == "yahoo" && host == "r.search.yahoo.com" {
        let (_, value) = raw.path().split_once("/RU=")?;
        let end = ["/RS=", "/RK="].iter().filter_map(|marker| value.find(marker)).min().unwrap_or(value.len());
        // form_urlencoded supplies percent decoding. Escape literal '+' first.
        url::form_urlencoded::parse(format!("url={}", value[..end].replace('+', "%2B")).as_bytes()).next()?.1.into_owned()
    } else {
        raw.to_string()
    };
    let url = crate::fetch::validated_url(&decoded).ok()?;
    (!paid_url(&url)).then_some(url)
}

fn parse(provider: &str, html: &str, limit: usize) -> Result<Vec<SearchResult>> {
    let format = layout(provider)?;
    let base = Url::parse(format.endpoint)?;
    let cards = selector(format.result)?;
    let links = selector(format.link)?;
    let titles = selector(format.title)?;
    let snippets = selector(format.snippet)?;
    let document = Html::parse_document(html);
    let mut results = Vec::new();
    for card in document.select(&cards) {
        if results.len() >= limit { break; }
        if paid_context(card, &titles, &snippets) { continue; }
        let Some(link) = card.select(&links).next() else { continue; };
        let Some(url) = destination(provider, &base, link.value().attr("href").unwrap_or("")) else { continue; };
        let title = card.select(&titles).next().map(|el| el.text().collect::<String>().trim().to_owned()).unwrap_or_default();
        if title.is_empty() { continue; }
        let mut snippet = card.select(&snippets).next().map(|el| el.text().collect::<String>().trim().to_owned()).unwrap_or_default();
        if provider == "yahoo" { snippet = snippet.split_whitespace().collect::<Vec<_>>().join(" "); }
        results.push(SearchResult { title, url: url.into(), snippet, score: 0.0, providers: vec![], document_id: None });
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paid_cards_are_rejected_before_limits_for_all_four_formats() {
        let shapes = [
            ("duckduckgo", r#"<div class="result web-result MARK"><a class="result__a" href="URL">Advertising research</a><a class="result__snippet">Sponsored content is the topic.</a>EXTRA</div>"#),
            ("brave", r#"<div data-type="web" class="MARK"><a class="l1" href="URL"><div class="search-snippet-title">Advertising research</div></a><div class="generic-snippet">Sponsored content is the topic.</div>EXTRA</div>"#),
            ("startpage", r#"<div class="result MARK"><a class="result-title" href="URL"><h2 class="wgl-title">Advertising research</h2></a><p class="description">Sponsored content is the topic.</p>EXTRA</div>"#),
            ("yahoo", r#"<div class="algo-sr MARK"><div class="compTitle"><a href="URL"><h3><span>Advertising research</span></h3></a></div><div class="compText">Sponsored content is the topic.</div>EXTRA</div>"#),
        ];
        for (provider, shape) in shapes {
            let card = |mark: &str, extra: &str, url: &str| shape.replace("MARK", mark).replace("EXTRA", extra).replace("URL", url);
            let organic = card("download", "", "https://example.org/article?topic=ads&utm_source=newsletter");
            let html = format!("{}<div id='sponsored'>{organic}</div>{}{}{organic}",
                card("result--ad result--ad--small", "", "https://example.org/paid"),
                card("", "<span aria-label='Sponsored'>Ad</span>", "https://example.org/paid"),
                card("", "", "https://www.bing.com/aclick?x=1"));
            let results = parse(provider, &html, 1).unwrap();
            assert_eq!(results.len(), 1, "{provider}");
            assert_eq!(results[0].url, "https://example.org/article?topic=ads&utm_source=newsletter", "{provider}");
            assert_eq!(results[0].title, "Advertising research");
            assert_eq!(results[0].snippet, "Sponsored content is the topic.");
            assert!(parse(provider, &card("", "<a rel='nofollow sponsored'>Details</a>", "https://example.org/paid"), 10).unwrap().is_empty());
            assert!(parse(provider, &card("", "<span>Sponsored</span>", "https://example.org/paid"), 10).unwrap().is_empty());
            assert_eq!(parse(provider, &organic.replace("Advertising research", "<span>Ad</span>"), 1).unwrap().len(), 1);
        }
    }

    #[test]
    fn redirect_context_and_destination_are_both_checked() {
        let ddg = Url::parse("https://html.duckduckgo.com/html/").unwrap();
        let yahoo = Url::parse("https://search.yahoo.com/search").unwrap();
        assert_eq!(destination("duckduckgo", &ddg, "/l/?uddg=https%3A%2F%2Fexample.org%2F%3Fx%3D1%26x%3D2").unwrap().as_str(), "https://example.org/?x=1&x=2");
        assert_eq!(destination("yahoo", &yahoo, "https://r.search.yahoo.com/_ylt=x/RU=https%3A%2F%2Fexample.org%2F%3Fx%3D1/RK=2/RS=x").unwrap().as_str(), "https://example.org/?x=1");
        for href in ["//duckduckgo.com/y.js?ad_provider=bingv7aa", "/l/?ad_provider=bingv7aa&uddg=https%3A%2F%2Fexample.org", "/l/?uddg=https%3A%2F%2Fwww.bing.com%2Faclick%3Fx%3D1", "https://example.org/?gclid=paid", "https://example.org/?utm_medium=cpc", "https://googleads.g.doubleclick.net/click"] {
            assert!(destination("duckduckgo", &ddg, href).is_none(), "{href}");
        }
        assert!(destination("brave", &ddg, "/relative-ad").is_none());
        for href in ["https://example.org/ads", "https://ads.google.com/", "https://notbing.com/aclick", "https://bing.com.example.org/aclick"] {
            assert!(destination("brave", &ddg, href).is_some(), "{href}");
        }
    }
}
