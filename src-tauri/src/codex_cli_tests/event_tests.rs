use super::*;

#[test]
fn dispatch_codex_command_events_maps_to_bash_events() {
    let mut events = Vec::new();
    let started = serde_json::json!({
        "type": "item.started",
        "item": {
            "id": "item_1",
            "type": "command_execution",
            "command": "/bin/zsh -lc pwd"
        }
    });
    let completed = serde_json::json!({
        "type": "item.completed",
        "item": {
            "id": "item_1",
            "type": "command_execution",
            "aggregated_output": "/private/tmp\n"
        }
    });

    dispatch_codex_event(&started, &mut |event| events.push(event));
    dispatch_codex_event(&completed, &mut |event| events.push(event));

    assert!(matches!(
        &events[0],
        AiAgentStreamEvent::ToolStart { tool_name, tool_id, .. }
            if tool_name == "Bash" && tool_id == "item_1"
    ));
    assert!(matches!(
        &events[1],
        AiAgentStreamEvent::ToolDone { tool_id, output }
            if tool_id == "item_1" && output.as_deref() == Some("/private/tmp\n")
    ));
}

#[test]
fn dispatch_codex_mcp_tool_call_maps_to_tool_events() {
    let mut events = Vec::new();
    let started = serde_json::json!({
        "type": "item.started",
        "item": {
            "id": "item_1",
            "type": "mcp_tool_call",
            "server": "nabu",
            "tool": "search_notes",
            "arguments": { "query": "meeting", "limit": 5 },
            "status": "in_progress"
        }
    });
    let completed = serde_json::json!({
        "type": "item.completed",
        "item": {
            "id": "item_1",
            "type": "mcp_tool_call",
            "server": "nabu",
            "tool": "search_notes",
            "arguments": { "query": "meeting", "limit": 5 },
            "result": [{ "title": "Meeting notes" }],
            "status": "completed"
        }
    });

    dispatch_codex_event(&started, &mut |event| events.push(event));
    dispatch_codex_event(&completed, &mut |event| events.push(event));

    assert!(matches!(
        &events[0],
        AiAgentStreamEvent::ToolStart { tool_name, tool_id, input }
            if tool_name == "search_notes"
                && tool_id == "item_1"
                && input.as_deref().is_some_and(|value| value.contains("meeting"))
    ));
    assert!(matches!(
        &events[1],
        AiAgentStreamEvent::ToolDone { tool_id, output }
            if tool_id == "item_1"
                && output.as_deref().is_some_and(|value| value.contains("Meeting notes"))
    ));
}

#[test]
fn dispatch_codex_agent_message_maps_to_text_delta() {
    let mut events = Vec::new();
    let completed = serde_json::json!({
        "type": "item.completed",
        "item": {
            "id": "item_2",
            "type": "agent_message",
            "text": "All set"
        }
    });

    dispatch_codex_event(&completed, &mut |event| events.push(event));

    assert!(matches!(
        &events[0],
        AiAgentStreamEvent::TextDelta { text } if text == "All set"
    ));
}

#[test]
fn format_codex_error_explains_vault_write_permission_failures() {
    let message = format_codex_error(CodexProcessError {
        stderr_output: "The patch was rejected by the environment: writing is blocked by read-only sandbox; rejected by user approval settings".into(),
        status: "exit status: 1".into(),
    });

    assert!(message.contains("active vault"));
    assert!(message.contains("writable"));
    assert!(message.contains("outside"));
}
