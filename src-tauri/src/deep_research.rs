use crate::ai_agents::{AiAgentId, AiAgentPermissionMode};
use serde::{Deserialize, Serialize};

pub const DEEP_RESEARCH_MIN_DEPTH: u32 = 1;
pub const DEEP_RESEARCH_MAX_DEPTH: u32 = 5;
const DEFAULT_DEPTH: u32 = 3;
const FINAL_REPORT_OPEN_TAG: &str = "<final-report>";
const FINAL_REPORT_CLOSE_TAG: &str = "</final-report>";
const MAX_INTERIM_SUMMARY_CHARS: usize = 600;

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
    },
    InterimSummary {
        iteration: u32,
        text: String,
    },
    Result {
        report: String,
    },
    Error {
        message: String,
    },
    Done,
}

fn research_system_prompt(query: &str) -> String {
    format!(
        "You are a research agent. Your goal is to thoroughly research '{query}'.\n\
         Use web scraping tools (Bash with curl, WebFetch, Read) to gather information from multiple sources.\n\
         After each tool call, decide if you need more sources or if you have enough to synthesize a final report.\n\
         Write your findings to NOTES.md in the vault as you go.\n\
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
        let message = build_iteration_message(&query, iteration, depth, &previous_summary);
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
                                emit(DeepResearchEvent::SourceAdded {
                                    title: source_title_from_url(&url),
                                    url,
                                    excerpt: truncate_excerpt(input, 140),
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
                                    emit(DeepResearchEvent::SourceAdded {
                                        title: source_title_from_url(&url),
                                        url,
                                        excerpt: truncate_excerpt(input, 140),
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
            emit(DeepResearchEvent::Result { report });
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
            emit(DeepResearchEvent::Result { report: fallback });
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