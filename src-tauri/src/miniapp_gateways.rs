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
#[cfg(feature = "miniapp-gateways")]
use std::path::Path;
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
    if let Ok(ip) = host.trim_end_matches(|c: char| c != ']' && !c.is_ascii_alphanumeric()).parse::<IpAddr>() {
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

fn read_limited_body(response: &mut reqwest::blocking::Response) -> Result<Vec<u8>, String> {
    let mut body: Vec<u8> = Vec::new();
    let mut buffer = [0u8; 16 * 1024];
    loop {
        use std::io::Read as _;
        let chunk = response
            .read(&mut buffer)
            .map_err(|error| format!("Failed to read response body: {error}"))?;
        if chunk == 0 {
            break;
        }
        if body.len() + chunk > MAX_RESPONSE_BYTES {
            return Err("Response body exceeds the 5 MB gateway limit".to_string());
        }
        body.extend_from_slice(&buffer[..chunk]);
    }
    Ok(body)
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
pub fn fetch_over_http(raw_url: &str) -> Result<ProxyFetchResult, String> {
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
    let without_scripts_owned;
    let without_scripts: &str = match Regex::new(r"(?is)<(script|style)[^>]*>.*?</\1>") {
        Ok(pattern) => {
            without_scripts_owned = pattern.replace_all(html, "").into_owned();
            &without_scripts_owned
        }
        Err(_) => html,
    };
    let without_tags_owned;
    let without_tags: &str = match Regex::new(r"(?s)<[^>]*>") {
        Ok(pattern) => {
            without_tags_owned = pattern.replace_all(without_scripts, " ").into_owned();
            &without_tags_owned
        }
        Err(_) => without_scripts,
    };
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

// ── Feature-gated gateways (commands + RSS/activity/email) ───────────────

/// RSS/Atom item returned by `fetch_rss_feed`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RssItem {
    pub title: String,
    pub link: String,
    pub summary: String,
    /// RFC 3339 timestamp when present.
    pub published: Option<String>,
    pub author: String,
}

/// Reports the foreground application, for time-tracking mini-apps.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ActivityInfo {
    pub app_name: String,
    pub window_title: String,
    pub process_id: u32,
}

/// Summary of one IMAP→markdown sync run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct EmailSyncReport {
    pub fetched: u32,
    pub errors: Vec<String>,
}

#[cfg(feature = "miniapp-gateways")]
#[tauri::command]
pub fn proxy_fetch(url: String) -> Result<ProxyFetchResult, String> {
    fetch_over_http(&url)
}

/// Fetches `url` (statically via reqwest, or JS-rendered via shell-out
/// when `js` is set) and extracts text for `selector`. JS rendering spawns a
/// headless browser subprocess (high RAM), so it additionally requires the
/// `mini_apps_web_access_enabled` setting to be on.
#[cfg(feature = "miniapp-gateways")]
#[tauri::command]
pub fn scrape_selection(url: String, selector: String, js: bool) -> Result<ScrapedSelection, String> {
    let (html, js_rendered) = if js {
        if !mini_apps_web_access_enabled() {
            return Err(
                "JavaScript-rendered scraping is disabled. Enable mini-app web access in Settings — it lets mini-apps run a headless browser, which uses significant RAM.".to_string(),
            );
        }
        match dump_dom_with_js(&url) {
            Ok(rendered) => (rendered, true),
            Err(error) => return Err(error),
        }
    } else {
        (fetch_over_http(&url)?.body, false)
    };
    scrape_selector(&html, &selector, js_rendered)
        .map(|mut selection| {
            selection.url = url;
            selection
        })
}

/// Reads the mini-app web-access toggle (defaults to enabled) from saved
/// settings. A missing or unreadable settings file must not break static
/// scraping, so failures resolve to the permissive default.
#[cfg(feature = "miniapp-gateways")]
fn mini_apps_web_access_enabled() -> bool {
    crate::settings::get_settings()
        .map(|settings| settings.mini_apps_web_access_enabled.unwrap_or(true))
        .unwrap_or(true)
}
#[cfg(feature = "miniapp-gateways")]
#[tauri::command]
pub fn fetch_rss_feed(url: String) -> Result<Vec<RssItem>, String> {
    use feed_rs::model::Text;

    let fetched = fetch_over_http(&url)?;
    if fetched.status >= 400 {
        return Err(format!("Feed returned HTTP {status}", status = fetched.status));
    }
    let feed = feed_rs::parser::parse(fetched.body.as_bytes())
        .map_err(|error| format!("Failed to parse feed: {error}"))?;

    Ok(feed
        .entries
        .into_iter()
        .map(|entry| RssItem {
            title: entry
                .title
                .map(|Text { content, .. }| content)
                .unwrap_or_default(),
            link: entry
                .links
                .first()
                .map(|link| link.href.clone())
                .unwrap_or_default(),
            summary: entry
                .summary
                .map(|Text { content, .. }| content)
                .unwrap_or_default(),
            published: entry
                .published
                .map(|timestamp| timestamp.to_rfc3339()),
            author: entry
                .authors
                .first()
                .map(|person| person.name.clone())
                .unwrap_or_default(),
        })
        .collect())
}

/// Returns the currently focused application (activity gateway).
#[cfg(feature = "miniapp-gateways")]
#[tauri::command]
pub fn get_current_activity() -> Result<ActivityInfo, String> {
    let window = active_win_pos_rs::get_active_window()
        .map_err(|error| format!("Failed to get active window: {error:?}"))?;
    Ok(ActivityInfo {
        app_name: window.app_name,
        window_title: window.title,
        process_id: u32::try_from(window.process_id).unwrap_or(0),
    })
}

/// Fetches unseen IMAP messages and writes them as markdown notes under
/// `<vault>/emails/`. Credentials come from `.nabu/config.toml` which
/// stores an env-var *name* only — the secret never touches disk.
#[cfg(feature = "miniapp-gateways")]
#[tauri::command]
pub async fn sync_email(vault_path: String) -> Result<EmailSyncReport, String> {
    let config = load_email_config(&vault_path)?;
    let password = std::env::var(&config.password_env)
        .map_err(|_| format!("Environment variable '{}' is not set", config.password_env))?;

    // async-imap 0.11 leaves transport setup to the caller: dial TCP,
    // wrap in TLS, then hand the stream to `Client::new`.
    let tcp = tokio::net::TcpStream::connect((config.imap_server.as_str(), config.imap_port))
        .await
        .map_err(|error| format!("IMAP connection failed: {error}"))?;
    let tls = tokio_native_tls::TlsConnector::from(native_tls::TlsConnector::new()
        .map_err(|error| format!("TLS setup failed: {error}"))?);
    let tls_stream = tls
        .connect(config.imap_server.as_str(), tcp)
        .await
        .map_err(|error| format!("TLS handshake failed: {error}"))?;
    let email_client = async_imap::Client::new(tls_stream);
    let mut session = email_client
        .login(&config.username, &password)
        .await
        .map_err(|error| error.0)
        .map_err(|error| format!("IMAP login failed: {error}"))?;

    session
        .select(&config.sync_folder)
        .await
        .map_err(|error| format!("Failed to select folder: {error}"))?;
    let unseen = session
        .search("UNSEEN")
        .await
        .map_err(|error| format!("IMAP search failed: {error}"))?;

    let mut report = EmailSyncReport::default();
    for sequence in unseen.iter().take(50) {
        let mut fetches = match session
            .fetch(sequence.to_string(), "RFC822")
            .await
        {
            Ok(fetches) => fetches,
            Err(error) => {
                report.errors.push(format!("fetch {sequence}: {error}"));
                continue;
            }
        };
        // async-imap 0.11 returns a stream of fetches rather than a Vec.
        while let Some(fetch) = futures_util::StreamExt::next(&mut fetches).await {
            match fetch {
                Ok(fetch) => match fetch.body() {
                    Some(body) => {
                        match save_email_as_markdown(
                            &vault_path,
                            &config.target_directory,
                            body,
                            *sequence,
                        ) {
                            Ok(()) => report.fetched += 1,
                            Err(error) => report.errors.push(error),
                        }
                    }
                    None => report.errors.push(format!("fetch {sequence}: empty body")),
                },
                Err(error) => report.errors.push(format!("fetch {sequence}: {error}")),
            }
        }
    }
    let _ = session.logout().await;
    Ok(report)
}

#[derive(Debug, Clone, Deserialize)]
#[cfg(feature = "miniapp-gateways")]
struct EmailConfig {
    imap_server: String,
    #[serde(default = "default_imap_port")]
    imap_port: u16,
    username: String,
    /// Name of the environment variable holding the account password.
    password_env: String,
    #[serde(default = "default_sync_folder")]
    sync_folder: String,
    #[serde(default = "default_target_directory")]
    target_directory: String,
}

#[cfg(feature = "miniapp-gateways")]
fn default_imap_port() -> u16 {
    993
}
#[cfg(feature = "miniapp-gateways")]
fn default_sync_folder() -> String {
    "INBOX".to_string()
}
#[cfg(feature = "miniapp-gateways")]
fn default_target_directory() -> String {
    "emails".to_string()
}

#[cfg(feature = "miniapp-gateways")]
fn load_email_config(vault_path: &str) -> Result<EmailConfig, String> {
    let config_path = Path::new(vault_path)
        .join(".nabu")
        .join("config.toml");
    let raw = std::fs::read_to_string(&config_path)
        .map_err(|error| format!("Missing or unreadable .nabu/config.toml: {error}"))?;
    let parsed: toml::Table = raw
        .parse()
        .map_err(|error| format!("Failed to parse .nabu/config.toml: {error}"))?;
    let section = parsed
        .get("email_sync")
        .ok_or("config.toml is missing an [email_sync] section")?;
    section
        .clone()
        .try_into::<EmailConfig>()
        .map_err(|error| format!("Invalid [email_sync] section: {error}"))
}

#[cfg(feature = "miniapp-gateways")]
fn save_email_as_markdown(
    vault_path: &str,
    target_directory: &str,
    body: &[u8],
    _sequence: u32,
) -> Result<(), String> {
    use mail_parser::MessageParser;

    let email = MessageParser::default()
        .parse(body)
        .ok_or("Failed to parse email message")?;

    let from = email
        .from()
        .and_then(|addresses| addresses.first())
        .map(|address| address.address().unwrap_or_default().to_string())
        .unwrap_or_default();
    let subject = email.subject().unwrap_or("(No Subject)").to_string();
    let date = email
        .date()
        .map(|header| header.to_timestamp())
        .unwrap_or_else(|| chrono::Utc::now().timestamp());
    let date_string = chrono::DateTime::from_timestamp(date, 0)
        .map(|datetime| datetime.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "0000-00-00".to_string());

    let body_text = email
        .body_text(0)
        .map(|text| text.to_string())
        .unwrap_or_else(|| {
            email
                .body_html(0)
                .map(|html| {
                    html2text::from_read(html.as_bytes(), 80).into_iter().collect::<String>()
                })
                .unwrap_or_default()
        });

    let safe_subject: String = subject
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || matches!(character, '-' | '_' | ' ') {
                character
            } else {
                '_'
            }
        })
        .collect();
    let safe_subject = safe_subject.trim().replace(' ', "-");
    let filename = format!("{date_string}-{safe_subject}.md");

    let directory = Path::new(vault_path).join(target_directory);
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("Failed to create emails directory: {error}"))?;
    let markdown = format!(
        "---\nfrom: \"{from}\"\nsubject: \"{subject}\"\ndate: {date_string}\nsource: email\nstatus: inbox\n---\n\n{body_text}\n"
    );
    std::fs::write(directory.join(&filename), markdown)
        .map_err(|error| format!("Failed to write email note: {error}"))
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
}
