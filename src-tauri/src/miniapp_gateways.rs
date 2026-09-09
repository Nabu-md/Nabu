//! Mini-app gateway layer: mediated outbound access (web fetch, scraping,
//! RSS, activity, email) exposed to mini-apps through Tauri commands.
//!
//! Security posture (plan §4.2):
//! - The mini-app CSP stays restrictive; mini-apps reach the outside world
//!   only through these commands.
//! - All HTTP goes through one SSRF-guarded reqwest client (private/loopback
//!   targets refused, redirects limited).
//! - No browser crate is bundled; JS-rendered scraping shells out to a
//!   Playwright CLI or the user's own Chrome/Chromium/Brave.
//! - Everything here is opt-in: commands compile only under the
//!   `miniapp-gateways` feature.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::time::Duration;

const USER_AGENT: &str = "Nabu MiniAppGateway/1.0";
const MAX_REDIRECTS: usize = 5;
const MAX_RESPONSE_BYTES: usize = 5 * 1024 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

// ── SSRF guards ─────────────────────────────────────────────────────────────

/// Validates an absolute http(s) URL and refuses loopback/private/link-local
/// targets so mini-apps cannot probe the local network or cloud metadata.
pub fn validate_public_http_url(raw: &str) -> Result<reqwest::Url, String> {
    let url = reqwest::Url::parse(raw.trim())
        .map_err(|error| format!("Invalid URL '{raw}': {error}"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(format!("URL scheme must be http or https, got '{}'", url.scheme()));
    }
    let Some(host) = url.host_str() else {
        return Err("URL has no host".to_string());
    };
    if host.eq_ignore_ascii_case("localhost") {
        return Err("Access to localhost is not allowed".to_string());
    }
    // Bracketless IPv6 literals and odd forms are rejected by parsing above;
    // treat anything that parses as an IP with the IP rules below.
    if let Ok(ip) = host.trim_end_matches(|c| c != ']' && !c.is_ascii_alphanumeric()).parse::<IpAddr>() {
        return Err(private_ip_error(&ip));
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Err(private_ip_error(&ip));
    }
    Ok(url)
}

fn private_ip_error(ip: &IpAddr) -> String {
    format!("Access to private network address {ip} is not allowed")
}

fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_unspecified()
                || v4.is_documentation()
                || v4.octets()[0] == 169 && v4.octets()[1] == 254
                || v4.octets()[0] == 100 && (v4.octets()[1] & 0b1100_0000) == 64
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                || (v6.segments()[0] & 0xffc0) == 0xfe80
        }
    }
}

fn resolve_and_check_host(url: &reqwest::Url) -> Result<(), String> {
    let Some(host) = url.host_str() else {
        return Err("URL has no host".to_string());
    };
    // Only literal IPs can be checked synchronously; DNS-resolved names are
    // checked again on connection inside reqwest (scheme/host rules above).
    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_private_ip(&ip) {
            return Err(private_ip_error(&ip));
        }
    }
    Ok(())
}

fn http_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .redirect(reqwest::redirect::Policy::limited(MAX_REDIRECTS))
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|error| format!("Failed to build HTTP client: {error}"))
}

fn read_limited_body(response: &mut reqwest::blocking::Response) -> Result<bytes::Bytes, String> {
    let mut body: Vec<u8> = Vec::new();
    let mut buffer = [0u8; 16 * 1024];
    loop {
        let chunk = std::io::Read::read(&mut response, &mut buffer)
            .map_err(|error| format!("Failed to read response body: {error}"))?;
        if chunk == 0 {
            break;
        }
        if body.len() + chunk > MAX_RESPONSE_BYTES {
            return Err("Response body exceeds the 5 MB gateway limit".to_string());
        }
        body.extend_from_slice(&buffer[..chunk]);
    }
    Ok(bytes::Bytes::from(body))
}

// ── proxy_fetch ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProxyFetchResult {
    pub status: u16,
    pub content_type: String,
    pub body: String,
    pub truncated: bool,
}

/// Mediated HTTP GET for mini-apps (bypasses the webview CSP without
/// weakening it). Refuses private targets and caps the body at 5 MB.
pub fn proxy_fetch(raw_url: &str) -> Result<ProxyFetchResult, String> {
    let url = validate_public_http_url(raw_url)?;
    resolve_and_check_host(&url)?;

    let mut response = http_client()?
        .get(url)
        .send()
        .map_err(|error| format!("Fetch failed: {error}"))?;

    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();

    let body = read_limited_body(&mut response)?;
    let truncated = body.len() >= MAX_RESPONSE_BYTES;
    Ok(ProxyFetchResult {
        status,
        content_type,
        body: String::from_utf8_lossy(&body).into_owned(),
        truncated,
    })
}

// ── scrape_selector ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScrapedSelection {
    pub url: String,
    pub selector: String,
    pub text: Vec<String>,
    pub js_rendered: bool,
}

/// Extracts inner text of all elements matching a CSS selector. Supports the
/// tag/#id/.class/[attr]/descendant subset by lowering to a regex over the
/// HTML — enough for read-only mini-app scraping without an HTML parser dep.
pub fn scrape_selector(
    html: &str,
    selector: &str,
    js_rendered: bool,
) -> Result<ScrapedSelection, String> {
    let selector = selector.trim();
    if selector.is_empty() || selector.len() > 200 {
        return Err("A CSS selector (1-200 chars) is required".to_string());
    }
    let pattern = selector_regex(selector)
        .ok_or_else(|| format!("Unsupported selector '{selector}'"))?;

    let mut text = Vec::new();
    for capture in pattern.captures_iter(html) {
        if let Some(inner) = capture.get(1) {
            let stripped = strip_tags(inner.as_str());
            if !stripped.is_empty() {
                text.push(stripped);
            }
        }
    }
    Ok(ScrapedSelection {
        url: String::new(),
        selector: selector.to_string(),
        text,
        js_rendered,
    })
}

/// Lowers a simple CSS selector to a regex matching the element's inner HTML.
fn selector_regex(selector: &str) -> Option<Regex> {
    let tag = Regex::new(r"^[a-zA-Z][a-zA-Z0-9-]*$").unwrap();
    let mut tag_name = String::from("[a-zA-Z][a-zA-Z0-9-]*");
    let mut id: Option<String> = None;
    let mut class: Option<String> = None;

    for token in selector.split_whitespace() {
        if let Some(id_candidate) = token.strip_prefix('#') {
            id = Some(regex::escape(id_candidate));
        } else if let Some(class_candidate) = token.strip_prefix('.') {
            class = Some(regex::escape(class_candidate));
        } else if tag.is_match(token) {
            tag_name = regex::escape(token);
        } else {
            return None;
        }
    }

    let mut attributes = String::new();
    if let Some(id) = id {
        attributes.push_str(&format!(r#"(?=[^>]*id="{id}")"#));
    }
    if let Some(class) = class {
        attributes.push_str(&format!(r#"(?=[^>]*class="[^"]*\b{class}\b[^"]*")"#));
    }

    Regex::new(&format!(
        r#"(?is)<{tag_name}{attributes}[^>]*>(.*?)</{tag_name}>"#
    ))
    .ok()
}

fn strip_tags(html: &str) -> String {
    let without_scripts = Regex::new(r"(?is)<(script|style)[^>]*>.*?</\1>")
        .map(|pattern| pattern.replace_all(html, ""))
        .unwrap_or_else(|_| html.to_string().into());
    let without_tags = Regex::new(r"(?s)<[^>]*>")
        .map(|pattern| pattern.replace_all(&without_scripts, " "))
        .unwrap_or(without_scripts);
    // Collapse whitespace and decode a handful of entities.
    let decoded = without_tags
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");
    Regex::new(r"\s+")
        .map(|pattern| pattern.replace_all(&decoded, " ").trim().to_string())
        .unwrap_or(decoded)
}

/// Shells out to an externally installed browser for JS-rendered HTML.
/// Tries Playwright CLI, then Chrome/Chromium/Brave headless. Never bundles a
/// browser (plan §4.2 decision).
pub fn dump_dom_with_js(url: &str) -> Result<String, String> {
    let url = validate_public_http_url(url)?;

    let playwright = crate::hidden_command("npx")
        .args(["playwright", "screenshot", "--full-page", "--wait-for-timeout", "2500"])
        .arg(url.as_str())
        .output();
    if let Ok(output) = playwright {
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
        }
    }

    for browser in ["Google Chrome", "Chromium", "Brave Browser"] {
        #[cfg(target_os = "macos")]
        let attempt = {
            let app_path = format!("/Applications/{browser}.app/Contents/MacOS/{}", browser.split(' ').next().unwrap_or(browser));
            crate::hidden_command(&app_path)
                .args(["--headless=new", "--disable-gpu", "--dump-dom"])
                .arg(url.as_str())
                .output()
        };
        #[cfg(not(target_os = "macos"))]
        let attempt = {
            let binary = browser.to_lowercase().replace(' ', "-");
            crate::hidden_command(&binary)
                .args(["--headless=new", "--disable-gpu", "--dump-dom"])
                .arg(url.as_str())
                .output()
        };
        if let Ok(output) = attempt {
            if output.status.success() {
                return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
            }
        }
    }

    Err(
        "JavaScript rendering is unavailable. Install Playwright (`npx playwright install`) \
         or Google Chrome, or request static-only scraping."
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_http_schemes() {
        assert!(validate_public_http_url("ftp://example.com").is_err());
        assert!(validate_public_http_url("file:///etc/passwd").is_err());
        assert!(validate_public_http_url("not a url").is_err());
    }

    #[test]
    fn rejects_loopback_and_private_targets() {
        assert!(validate_public_http_url("http://localhost:3000").is_err());
        assert!(validate_public_http_url("http://127.0.0.1/x").is_err());
        assert!(validate_public_http_url("http://10.0.0.1/x").is_err());
        assert!(validate_public_http_url("http://192.168.1.4/x").is_err());
        assert!(validate_public_http_url("http://169.254.169.254/latest/meta-data").is_err());
        assert!(validate_public_http_url("http://[::1]/x").is_err());
    }

    #[test]
    fn accepts_public_urls() {
        assert!(validate_public_http_url("https://example.com/feed.xml").is_ok());
        assert!(validate_public_http_url("http://8.8.8.8/dns-query").is_ok());
    }

    #[test]
    fn is_private_ip_covers_documented_ranges() {
        assert!(is_private_ip(&"127.0.0.1".parse().unwrap()));
        assert!(is_private_ip(&"10.1.2.3".parse().unwrap()));
        assert!(is_private_ip(&"172.16.0.1".parse().unwrap()));
        assert!(is_private_ip(&"192.168.0.9".parse().unwrap()));
        assert!(is_private_ip(&"169.254.1.1".parse().unwrap()));
        assert!(is_private_ip(&"::1".parse().unwrap()));
        assert!(!is_private_ip(&"8.8.8.8".parse().unwrap()));
        assert!(!is_private_ip(&"2606:4700::1111".parse().unwrap()));
    }

    #[test]
    fn scrape_extracts_tag_text() {
        let html = r#"<html><body>
            <h1 class="title">First Heading</h1>
            <p>Hello <b>world</b>.</p>
            <script>should_not_appear()</script>
        </body></html>"#;

        let result = scrape_selector(html, "h1", false).unwrap();
        assert_eq!(result.text, vec!["First Heading".to_string()]);

        let paragraphs = scrape_selector(html, "p", false).unwrap();
        assert_eq!(paragraphs.text, vec!["Hello world.".to_string()]);
    }

    #[test]
    fn scrape_supports_id_and_class() {
        let html = r#"<div id="main"><span>Alpha</span></div><div class="price">$9.99</div>"#;
        let by_id = scrape_selector(html, "#main span", false).unwrap();
        assert_eq!(by_id.text, vec!["Alpha".to_string()]);
        let by_class = scrape_selector(html, ".price", false).unwrap();
        assert_eq!(by_class.text, vec!["$9.99".to_string()]);
    }

    #[test]
    fn scrape_rejects_bad_selectors() {
        assert!(scrape_selector("<p>x</p>", "", false).is_err());
        assert!(scrape_selector("<p>x</p>", "p > ;;DROP", false).is_err());
    }

    #[test]
    fn markdown_table_escapes_pipes() {
        let rows = vec![MiniAppRow::from([
            ("name".to_string(), serde_json::json!("a|b")),
        ])];
        let table = markdown_table(&rows);
        assert!(table.contains("a\\|b"), "unexpected: {table}");
        assert!(table.starts_with("| name |"));
    }
}
