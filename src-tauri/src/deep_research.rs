use crate::ai_agents::{AiAgentId, AiAgentPermissionMode};
use serde::{Deserialize, Serialize};

pub const DEEP_RESEARCH_MIN_DEPTH: u32 = 1;
pub const DEEP_RESEARCH_MAX_DEPTH: u32 = 5;
const DEFAULT_DEPTH: u32 = 3;
const FINAL_REPORT_OPEN_TAG: &str = "<final-report>";
const FINAL_REPORT_CLOSE_TAG: &str = "</final-report>";
const MAX_INTERIM_SUMMARY_CHARS: usize = 600;
const DEFAULT_REPORT_NOTE_PATH: &str = "Research Reports";
const RESEARCH_NOTES_FILENAME: &str = "NOTES.md";

#[derive(Debug, Clone, Deserialize)]
pub struct DeepResearchRequest {
    pub query: String,
    pub vault_path: String,
    #[serde(default)]
    pub vault_paths: Vec<String>,
    #[serde(default)]
    pub depth: Option<u32>,
    #[serde(default)]
    pub model: Option<String>,
    /// Which CLI agent runs the research loop. Defaults to Claude Code.
    #[serde(default)]
    pub agent: Option<AiAgentId>,
    #[serde(default)]
    pub permission_mode: Option<AiAgentPermissionMode>,
    #[serde(default)]
    pub event_name: Option<String>,
    /// Buzz team channel for multiplayer research. When set, recent team
    /// messages are injected before each iteration and progress updates plus
    /// the final report are posted to the channel.
    #[serde(default)]
    pub team_channel: Option<String>,
}

impl DeepResearchRequest {
    pub(crate) fn effective_agent(&self) -> AiAgentId {
        self.agent.unwrap_or(AiAgentId::ClaudeCode)
    }
}

impl DeepResearchRequest {
    pub(crate) fn effective_depth(&self) -> u32 {
        self.depth
            .unwrap_or(DEFAULT_DEPTH)
            .clamp(DEEP_RESEARCH_MIN_DEPTH, DEEP_RESEARCH_MAX_DEPTH)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind")]
pub enum DeepResearchEvent {
    IterationStart {
        iteration: u32,
        goal: String,
    },
    ToolStart {
        tool_name: String,
        tool_id: String,
    },
    ToolDone {
        tool_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<String>,
    },
    SourceAdded {
        title: String,
        url: String,
        excerpt: String,
        /// 0-1 trust/quality estimate used by the frontend to sort sources.
        relevance_score: f64,
    },
    InterimSummary {
        iteration: u32,
        text: String,
    },
    Result {
        report: String,
    },
    /// The final report was persisted into the vault as a markdown note.
    ReportWritten {
        path: String,
    },
    Error {
        message: String,
    },
    Done,
}

/// Structured research methodology folded into the system prompt (question
/// decomposition → search planning → per-question sourcing → cross-reference
/// → synthesis). The multi-iteration loop stays; each iteration now follows
/// this pipeline instead of free-form scraping.
fn research_system_prompt(query: &str) -> String {
    format!(
        "You are a research agent. Your goal is to thoroughly research '{query}'.\n\
         Use web scraping tools (Bash with curl/wget, the web_fetch MCP tool, WebFetch, Read) to gather information from multiple sources.\n\
         \n\
         Follow this structured research pipeline:\n\
         1. Decompose the research query into 2-4 focused sub-questions.\n\
         2. Generate a search plan per sub-question (search terms, likely source types).\n\
         3. Gather sources per sub-question; prefer primary and authoritative sources.\n\
         4. Cross-reference findings across sources; note agreements and contradictions.\n\
         5. Synthesize the final report from the cross-referenced findings.\n\
         \n\
         Write your findings to {RESEARCH_NOTES_FILENAME} in the vault as you go.\n\
         At the very end of your final response, output your final report wrapped in <final-report> tags."
    )
}

fn build_iteration_message(query: &str, iteration: u32, depth: u32, previous_summary: &str) -> String {
    let scope = if iteration == 1 {
        "Gather information from multiple sources using web scraping tools. Identify the key facts, figures, and viewpoints."
    } else if iteration == depth {
        "This is your final iteration. Review everything you have gathered so far, close any remaining gaps, and synthesize a complete final report wrapped in <final-report> tags."
    } else {
        "Review your findings so far, identify gaps in your research, and gather additional information to fill those gaps. If you already have enough to synthesize a complete report, output it wrapped in <final-report> tags."
    };
    let context = if previous_summary.is_empty() {
        String::new()
    } else {
        format!(
            "\n\nFindings gathered so far (iteration {}/{}):\n{}\n",
            iteration - 1,
            depth,
            previous_summary
        )
    };
    format!(
        "Research query: {query}\n\nIteration {iteration} of {depth}. {scope}{context}"
    )
}

fn extract_final_report(text: &str) -> Option<String> {
    let open = text.find(FINAL_REPORT_OPEN_TAG)?;
    let after_open = &text[open + FINAL_REPORT_OPEN_TAG.len()..];
    let close = after_open.find(FINAL_REPORT_CLOSE_TAG)?;
    let report = after_open[..close].trim();
    if report.is_empty() {
        return None;
    }
    Some(report.to_string())
}

fn url_from_tool_input(input: &str) -> Option<String> {    const URL_START: &str = "http://";
    let lower = input.to_ascii_lowercase();
    for prefix in [URL_START, "https://"] {
        let mut search_from = 0;
        while let Some(relative) = lower[search_from..].find(prefix) {
            let start = search_from + relative;
            let rest = &input[start..];
            let end = rest
                .find(|character: char| character.is_whitespace() || character == '"' || character == '\'' || character == '`')
                .unwrap_or(rest.len());
            let url = &rest[..end];
            if url.len() > prefix.len() + 3 {
                return Some(url.to_string());
            }
            search_from = start + prefix.len();
        }
    }
    None
}

fn source_title_from_url(url: &str) -> String {
    url.strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .and_then(|rest| rest.split('/').next())
        .filter(|host| !host.is_empty())
        .unwrap_or(url)
        .to_string()
}

/// Highest-trust domains get 1.0; unknown hosts get a neutral 0.5 baseline.
fn url_domain_trust(url: &str) -> f64 {
    let host = source_title_from_url(url)
        .to_ascii_lowercase()
        .trim_start_matches("www.")
        .to_string();
    if host.is_empty() {
        return 0.5;
    }
    for suffix in [
        ".gov",
        ".edu",
        ".int",
        ".mil",
        ".europa.eu",
        ".un.org",
        ".who.int",
        ".worldbank.org",
        ".oecd.org",
        ".arxiv.org",
        ".nature.com",
        ".science.org",
    ] {
        if host == suffix.trim_start_matches('.') || host.ends_with(suffix) {
            return 1.0;
        }
    }
    match host.as_str() {
        "en.wikipedia.org" | "wikipedia.org" => 0.9,
        "reuters.com" | "apnews.com" | "bbc.com" | "bbc.co.uk" | "npr.org" | "theguardian.com"
        | "ft.com" | "economist.com" | "bloomberg.com" | "nature.com" => 0.85,
        "nytimes.com" | "washingtonpost.com" | "wsj.com" | "cnbc.com" | "forbes.com"
        | "theatlantic.com" | "wired.com" | "arstechnica.com" => 0.75,
        "medium.com" | "substack.com" | "blogspot.com" | "wordpress.com" | "quora.com"
        | "reddit.com" => 0.3,
        _ => 0.5,
    }
}

/// Relevance score (0-1) for a scraped source: domain trust scaled up by
/// excerpt quality (how URL-dense and information-rich the tool input is).
fn source_relevance_score(url: &str, excerpt: &str) -> f64 {
    let trust = url_domain_trust(url);
    let quality = (excerpt.trim().chars().count() as f64 / 140.0).clamp(0.0, 1.0);
    let url_depth_bonus = if url.matches('/').count() > 3 { 0.05 } else { 0.0 };
    (trust * 0.7 + quality * 0.3 + url_depth_bonus).clamp(0.0, 1.0)
}

fn truncate_excerpt(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut truncated: String = text.chars().take(max_chars).collect();
    truncated.push('…');
    truncated
}

fn agent_cli_label(agent: AiAgentId) -> &'static str {
    match agent {
        AiAgentId::ClaudeCode => "Claude Code",
        AiAgentId::Codex => "Codex",
        AiAgentId::Copilot => "GitHub Copilot",
        AiAgentId::Opencode => "OpenCode",
        AiAgentId::Pi => "Pi",
        AiAgentId::Antigravity => "Antigravity",
        AiAgentId::Kiro => "Kiro",
        AiAgentId::Hermes => "Hermes",
    }
}

type SharedResearchRunner<F> =
    fn(crate::cli_agent_runtime::AgentStreamRequest, F) -> Result<String, String>;

/// Persist a final research report into the vault as a markdown note under
/// `Research Reports/`, using `create_note` semantics (never overwrites; falls
/// back to timestamped filenames on collision). Returns the vault-relative
/// note path on success.
fn write_report_note(vault_path: &str, query: &str, report: &str) -> Result<String, String> {
    if vault_path.trim().is_empty() {
        return Err("A vault path is required to save the report".to_string());
    }

    let slug = report_note_slug(query);
    let base_stem = format!("{}-{}", slug, chrono::Utc::now().format("%Y-%m-%d"));
    let directory = std::path::Path::new(vault_path).join(DEFAULT_REPORT_NOTE_PATH);
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("Failed to create {DEFAULT_REPORT_NOTE_PATH}/: {error}"))?;

    let mut path = directory.join(format!("{base_stem}.md"));
    let mut attempt = 1;
    while path.exists() {
        attempt += 1;
        path = directory.join(format!("{base_stem}-{attempt}.md"));
    }

    let frontmatter_title = report_note_title(query);
    let content = format!(
        "---\ntitle: \"{frontmatter_title}\"\ntype: Research Report\ncreated: {}\n---\n\n# {frontmatter_title}\n\n{report}\n",
        chrono::Utc::now().to_rfc3339(),
    );
    std::fs::write(&path, content)
        .map_err(|error| format!("Failed to write report note: {error}"))?;

    let relative = path
        .strip_prefix(vault_path)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string_lossy().into_owned());
    Ok(relative)
}

fn report_note_title(query: &str) -> String {
    let cleaned = query.trim().split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.is_empty() {
        return "Research Report".to_string();
    }
    let mut title: String = cleaned.chars().take(80).collect();
    if title.len() < cleaned.len() {
        title.push('…');
    }
    title.replace('"', "'")
}

fn report_note_slug(query: &str) -> String {
    let slug: String = query
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .take(3)
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() {
        "research-report".to_string()
    } else {
        slug
    }
}

/// Shared-runtime CLI runners (everything except Claude Code). Each agent CLI
/// exposes the same `run_agent_stream(AgentStreamRequest, FnMut(AiAgentStreamEvent))`
/// shape, so the research loop can dispatch to any installed agent.
fn shared_research_runner<F>(agent: AiAgentId) -> Option<SharedResearchRunner<F>>
where
    F: FnMut(crate::ai_agents::AiAgentStreamEvent),
{
    match agent {
        AiAgentId::ClaudeCode => None,
        AiAgentId::Codex => Some(crate::codex_cli::run_agent_stream),
        AiAgentId::Copilot => Some(crate::copilot_cli::run_agent_stream),
        AiAgentId::Opencode => Some(crate::opencode_cli::run_agent_stream),
        AiAgentId::Pi => Some(crate::pi_cli::run_agent_stream),
        AiAgentId::Antigravity => Some(crate::antigravity_cli::run_agent_stream),
        AiAgentId::Kiro => Some(crate::kiro_cli::run_agent_stream),
        AiAgentId::Hermes => Some(crate::hermes_cli::run_agent_stream),
    }
}

/// Runs the multi-step deep research loop against any supported CLI agent.
///
/// Each iteration spawns the agent's CLI (Claude Code via `claude -p` with a
/// research tool policy; other CLIs via their shared JSON stream runner) with
/// the research agent system prompt. The stream is forwarded as
/// `DeepResearchEvent`s; when the model emits a `<final-report>` block (or the
/// depth cap is reached) the report is emitted and the loop terminates.
/// Returns the last session id.
pub fn run_deep_research<F>(request: DeepResearchRequest, mut emit: F) -> Result<String, String>
where
    F: FnMut(DeepResearchEvent),
{
    let query = request.query.trim().to_string();
    if query.is_empty() {
        emit(DeepResearchEvent::Error {
            message: "Research query cannot be empty".into(),
        });
        return Err("Research query cannot be empty".into());
    }
    if request.vault_path.trim().is_empty() {
        emit(DeepResearchEvent::Error {
            message: "A vault path is required for deep research".into(),
        });
        return Err("A vault path is required for deep research".into());
    }

    let agent = request.effective_agent();
    let depth = request.effective_depth();
    let mut previous_summary = String::new();
    let mut session_id = String::new();

    for iteration in 1..=depth {
        let mut message = build_iteration_message(&query, iteration, depth, &previous_summary);
        // Multiplayer mode: prepend team context so the agent knows what the
        // rest of the team is doing, then announce the iteration start.
        if let Some(channel) = request.team_channel.as_deref() {
            match crate::buzz_integration::get_team_messages(channel, 10) {
                Ok(messages) if !messages.is_empty() => {
                    let context = messages
                        .iter()
                        .map(|message| format!("[{}] {}", message.author, message.content))
                        .collect::<Vec<_>>()
                        .join("\n");
                    message = format!("Team context:\n{context}\n\n{message}");
                }
                Ok(_) => {}
                Err(error) => {
                    emit(DeepResearchEvent::Error { message: error });
                }
            }
            if let Err(error) = crate::buzz_integration::post_agent_update(
                channel,
                &format!(
                    "Starting research iteration {iteration}/{depth} for: {query}"
                ),
            ) {
                emit(DeepResearchEvent::Error { message: error });
            }
        }
        emit(DeepResearchEvent::IterationStart {
            iteration,
            goal: message.clone(),
        });

        let agent_request = crate::cli_agent_runtime::AgentStreamRequest {
            message,
            model: request.model.clone(),
            system_prompt: Some(research_system_prompt(&query)),
            vault_path: request.vault_path.clone(),
            vault_paths: request.vault_paths.clone(),
            permission_mode: request.permission_mode.unwrap_or_default(),
        };

        let mut iteration_text = String::new();
        let run_result = match shared_research_runner(agent) {
            Some(runner) => runner(agent_request, |event| {
                match event {
                    crate::ai_agents::AiAgentStreamEvent::Init { session_id: id } => {
                        session_id = id;
                    }
                    crate::ai_agents::AiAgentStreamEvent::TextDelta { text } => {
                        iteration_text.push_str(&text);
                    }
                    crate::ai_agents::AiAgentStreamEvent::ThinkingDelta { .. } => {}
                    crate::ai_agents::AiAgentStreamEvent::ToolStart {
                        tool_name,
                        tool_id,
                        input,
                    } => {
                        emit(DeepResearchEvent::ToolStart {
                            tool_name,
                            tool_id: tool_id.clone(),
                        });
                        if let Some(input) = input.as_deref() {
                            if let Some(url) = url_from_tool_input(input) {
                                let excerpt = truncate_excerpt(input, 140);
                                let relevance_score = source_relevance_score(&url, &excerpt);
                                emit(DeepResearchEvent::SourceAdded {
                                    title: source_title_from_url(&url),
                                    url,
                                    excerpt,
                                    relevance_score,
                                });
                            }
                        }
                    }
                    crate::ai_agents::AiAgentStreamEvent::ToolDone { tool_id, output } => {
                        emit(DeepResearchEvent::ToolDone { tool_id, output });
                    }
                    crate::ai_agents::AiAgentStreamEvent::Error { message } => {
                        emit(DeepResearchEvent::Error { message: message.clone() });
                    }
                    crate::ai_agents::AiAgentStreamEvent::Done => {}
                }
            }),
            None => {
                // Research runs always grant Bash + curl/wget (see
                // research_agent in claude_invocation.rs), so web scraping works
                // in every permission mode.
                crate::claude_cli::run_research_stream(agent_request, |event| {
                    match event {
                        crate::claude_cli::ClaudeStreamEvent::Init { session_id: id } => {
                            session_id = id;
                        }
                        crate::claude_cli::ClaudeStreamEvent::TextDelta { text } => {
                            iteration_text.push_str(&text);
                        }
                        crate::claude_cli::ClaudeStreamEvent::ThinkingDelta { .. } => {}
                        crate::claude_cli::ClaudeStreamEvent::ToolStart {
                            tool_name,
                            tool_id,
                            input,
                        } => {
                            emit(DeepResearchEvent::ToolStart {
                                tool_name,
                                tool_id: tool_id.clone(),
                            });
                            if let Some(input) = input.as_deref() {
                                if let Some(url) = url_from_tool_input(input) {
                                    let excerpt = truncate_excerpt(input, 140);
                                    let relevance_score = source_relevance_score(&url, &excerpt);
                                    emit(DeepResearchEvent::SourceAdded {
                                        title: source_title_from_url(&url),
                                        url,
                                        excerpt,
                                        relevance_score,
                                    });
                                }
                            }
                        }
                        crate::claude_cli::ClaudeStreamEvent::ToolDone { tool_id, output } => {
                            emit(DeepResearchEvent::ToolDone { tool_id, output });
                        }
                        crate::claude_cli::ClaudeStreamEvent::Result { text, .. } => {
                            iteration_text.push_str(&text);
                        }
                        crate::claude_cli::ClaudeStreamEvent::Error { message } => {
                            emit(DeepResearchEvent::Error { message: message.clone() });
                        }
                        crate::claude_cli::ClaudeStreamEvent::Done => {}
                    }
                })
            }
        };

        match run_result {
            Ok(id) => session_id = id,
            Err(error) => {
                emit(DeepResearchEvent::Error {
                    message: error.clone(),
                });
                emit(DeepResearchEvent::Done);
                return Err(error);
            }
        }

        if let Some(report) = extract_final_report(&iteration_text) {
            emit(DeepResearchEvent::Result {
                report: report.clone(),
            });
            // Multiplayer mode: share the finished report with the team.
            if let Some(channel) = request.team_channel.as_deref() {
                if let Err(error) = crate::buzz_integration::post_research_result(channel, &query, &report) {
                    emit(DeepResearchEvent::Error { message: error });
                }
            }
            // Reports are first-class notes: persist the final report into the
            // vault so the user can read, link, and PDF-export it. A failed
            // write must not fail the research run itself.
            match write_report_note(&request.vault_path, &query, &report) {
                Ok(note_path) => {
                    emit(DeepResearchEvent::ReportWritten { path: note_path });
                }
                Err(error) => {
                    emit(DeepResearchEvent::Error {
                        message: format!("Failed to save report as a note: {error}"),
                    });
                }
            }
            emit(DeepResearchEvent::Done);
            return Ok(session_id);
        }

        let summary = truncate_excerpt(iteration_text.trim(), MAX_INTERIM_SUMMARY_CHARS);
        if !summary.is_empty() {
            emit(DeepResearchEvent::InterimSummary {
                iteration,
                text: summary.clone(),
            });
            previous_summary = summary;
        }

        if iteration == depth {
            let fallback = if iteration_text.trim().is_empty() {
                format!(
                    "The {agent} research agent finished without producing a report. Check that the CLI is installed and that web scraping tools are permitted in this permission mode.",
                    agent = agent_cli_label(agent)
                )
            } else {
                iteration_text.trim().to_string()
            };
            emit(DeepResearchEvent::Result {
                report: fallback.clone(),
            });
            if let Some(channel) = request.team_channel.as_deref() {
                if let Err(error) = crate::buzz_integration::post_research_result(channel, &query, &fallback) {
                    emit(DeepResearchEvent::Error { message: error });
                }
            }
            match write_report_note(&request.vault_path, &query, &fallback) {
                Ok(note_path) => {
                    emit(DeepResearchEvent::ReportWritten { path: note_path });
                }
                Err(error) => {
                    emit(DeepResearchEvent::Error {
                        message: format!("Failed to save report as a note: {error}"),
                    });
                }
            }
            emit(DeepResearchEvent::Done);
            return Ok(session_id);
        }
    }

    emit(DeepResearchEvent::Done);
    Ok(session_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_depth_is_clamped() {
        let request = |depth: Option<u32>| DeepResearchRequest {
            query: "q".into(),
            vault_path: "/tmp/vault".into(),
            vault_paths: Vec::new(),
            depth,
            model: None,
            agent: None,
            permission_mode: None,
            event_name: None,
            team_channel: None,
        };
        assert_eq!(request(None).effective_depth(), 3);
        assert_eq!(request(Some(1)).effective_depth(), 1);
        assert_eq!(request(Some(0)).effective_depth(), 1);
        assert_eq!(request(Some(99)).effective_depth(), 5);
    }

    #[test]
    fn extract_final_report_pulls_wrapped_block() {
        let text = "Some preamble\n<final-report>\n# Findings\n\nReport body\n</final-report>\ntrailing";
        assert_eq!(
            extract_final_report(text).as_deref(),
            Some("# Findings\n\nReport body")
        );
    }

    #[test]
    fn extract_final_report_returns_none_without_tags() {
        assert_eq!(extract_final_report("no tags here"), None);
        assert_eq!(extract_final_report("<final-report></final-report>"), None);
    }

    #[test]
    fn url_from_tool_input_finds_web_urls() {
        assert_eq!(
            url_from_tool_input("curl -s https://example.com/article"),
            Some("https://example.com/article".into())
        );
        assert_eq!(
            url_from_tool_input("WebFetch {\"url\":\"http://news.example.org/story?q=1\"}"),
            Some("http://news.example.org/story?q=1".into())
        );
        assert_eq!(url_from_tool_input("no urls here"), None);
    }

    #[test]
    fn source_title_from_url_extracts_host() {
        assert_eq!(
            source_title_from_url("https://www.news.example.com/deep/page"),
            "www.news.example.com"
        );
        assert_eq!(source_title_from_url("http://example.org"), "example.org");
    }

    #[test]
    fn system_prompt_mentions_query_and_tags() {
        let prompt = research_system_prompt("climate change");
        assert!(prompt.contains("climate change"));
        assert!(prompt.contains("<final-report>"));
    }

    #[test]
    fn iteration_message_includes_progress_and_context() {
        let first = build_iteration_message("q", 1, 3, "");
        assert!(first.contains("Iteration 1 of 3"));
        assert!(!first.contains("Findings gathered so far"));

        let later = build_iteration_message("q", 3, 3, "some findings");
        assert!(later.contains("Iteration 3 of 3"));
        assert!(later.contains("some findings"));
        assert!(later.contains("final report"));
    }

    #[test]
    fn source_relevance_score_orders_domains_sensibly() {
        let gov = source_relevance_score("https://www.cdc.gov/flu/overview", "curl -s https://www.cdc.gov/flu/overview");
        let blog = source_relevance_score("https://medium.com/some-post", "curl -s https://medium.com/some-post");
        let unknown = source_relevance_score("https://unknown-example.org/a", "curl");

        assert!(gov > 0.7, "government domains should score high: {gov}");
        assert!(blog < 0.5, "low-trust platforms should score low: {blog}");
        assert!(unknown >= 0.3 && unknown <= 0.7, "unknown hosts stay neutral: {unknown}");
    }

    #[test]
    fn report_note_slug_is_clean_and_bounded() {
        assert_eq!(report_note_slug("Impact of Climate Change on Agriculture!"), "impact-of-climate");
        assert_eq!(report_note_slug("   "), "research-report");
    }

    #[test]
    fn report_note_title_is_trimmed_and_bounded() {
        assert_eq!(report_note_title("  short   query  "), "short query");
        let long = report_note_title(&"word ".repeat(40));
        assert!(long.chars().count() <= 81);
    }

    #[test]
    fn write_report_note_creates_timestamped_markdown() {
        let vault = tempfile::tempdir().unwrap();
        let vault_path = vault.path().to_string_lossy().into_owned();

        let relative = write_report_note(&vault_path, "climate impact", "# Findings\n\nBody").unwrap();

        assert!(relative.starts_with("Research Reports/climate-impact-"));
        assert!(relative.ends_with(".md"));
        let content = std::fs::read_to_string(vault.path().join(&relative)).unwrap();
        assert!(content.contains("type: Research Report"));
        assert!(content.contains("# Findings"));

        // A second write must not overwrite the first note.
        let second = write_report_note(&vault_path, "climate impact", "# Second").unwrap();
        assert_ne!(relative, second);
    }

    #[test]
    fn write_report_note_requires_vault_path() {
        assert!(write_report_note("  ", "query", "report").is_err());
    }

    #[test]
    fn shared_research_runner_dispatches_every_agent_except_claude() {
        type Noop = fn(crate::ai_agents::AiAgentStreamEvent);

        assert!(shared_research_runner::<Noop>(AiAgentId::ClaudeCode).is_none());
        for agent in [
            AiAgentId::Codex,
            AiAgentId::Copilot,
            AiAgentId::Opencode,
            AiAgentId::Pi,
            AiAgentId::Antigravity,
            AiAgentId::Kiro,
            AiAgentId::Hermes,
        ] {
            assert!(
                shared_research_runner::<Noop>(agent).is_some(),
                "missing shared research runner for {agent:?}"
            );
        }
    }

    #[test]
    fn agent_cli_labels_cover_all_agents() {
        for agent in [
            AiAgentId::ClaudeCode,
            AiAgentId::Codex,
            AiAgentId::Copilot,
            AiAgentId::Opencode,
            AiAgentId::Pi,
            AiAgentId::Antigravity,
            AiAgentId::Kiro,
            AiAgentId::Hermes,
        ] {
            assert!(!agent_cli_label(agent).is_empty());
        }
        assert_eq!(agent_cli_label(AiAgentId::Pi), "Pi");
    }

    #[test]
    fn empty_query_fails_fast_with_error_event() {
        let request = DeepResearchRequest {
            query: "   ".into(),
            vault_path: "/tmp/vault".into(),
            vault_paths: Vec::new(),
            depth: None,
            model: None,
            agent: None,
            permission_mode: None,
            event_name: None,
            team_channel: None,
        };
        let mut events = Vec::new();
        let result = run_deep_research(request, |event| events.push(event));

        assert!(result.is_err());
        assert!(matches!(
            events.first(),
            Some(DeepResearchEvent::Error { .. })
        ));
    }
}