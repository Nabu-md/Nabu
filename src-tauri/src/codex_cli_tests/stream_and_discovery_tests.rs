use super::*;

#[cfg(unix)]
#[test]
fn run_codex_agent_stream_reads_ndjson_and_returns_thread_id() {
    let (thread_id, events) = run_codex_script(
        r#"printf '%s\n' '{"type":"thread.started","thread_id":"thread_1"}'
printf '%s\n' '{"type":"item.completed","item":{"id":"msg_1","type":"agent_message","text":"Done"}}'
"#,
    );

    assert_eq!(thread_id, "thread_1");
    assert_codex_text_flow(&events, "thread_1", "Done");
}

#[cfg(unix)]
#[test]
fn run_codex_agent_stream_uses_last_message_file_when_stream_has_no_text() {
    let (thread_id, events) = run_codex_script(
        r#"last_message=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--output-last-message" ]; then
shift
last_message="$1"
  fi
  shift
done
printf '%s\n' '{"type":"thread.started","thread_id":"thread_1"}'
printf '%s' 'Recovered final answer' > "$last_message"
"#,
    );

    assert_eq!(thread_id, "thread_1");
    assert_codex_text_flow(&events, "thread_1", "Recovered final answer");
}

#[cfg(unix)]
#[test]
fn run_codex_agent_stream_does_not_duplicate_last_message_file_after_text_event() {
    let (thread_id, events) = run_codex_script(
        r#"last_message=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--output-last-message" ]; then
shift
last_message="$1"
  fi
  shift
done
printf '%s\n' '{"type":"thread.started","thread_id":"thread_1"}'
printf '%s\n' '{"type":"item.completed","item":{"id":"msg_1","type":"agent_message","text":"Streamed answer"}}'
printf '%s' 'Recovered final answer' > "$last_message"
"#,
    );

    let text_events = events
        .iter()
        .filter(|event| matches!(event, AiAgentStreamEvent::TextDelta { .. }))
        .count();

    assert_eq!(thread_id, "thread_1");
    assert_eq!(text_events, 1);
    assert_codex_text_flow(&events, "thread_1", "Streamed answer");
}

#[cfg(unix)]
#[test]
fn run_codex_agent_stream_reports_nonzero_exit_errors() {
    let (thread_id, events) = run_codex_script(
        r#"printf '%s\n' '{"type":"thread.started","thread_id":"thread_1"}'
printf '%s\n' 'login required' >&2
exit 2
"#,
    );

    assert_eq!(thread_id, "thread_1");
    assert!(events.iter().any(|event| matches!(
        event,
        AiAgentStreamEvent::Error { message } if message.contains("not authenticated")
    )));
    assert!(matches!(events.last(), Some(AiAgentStreamEvent::Done)));
}

#[cfg(unix)]
#[test]
fn run_codex_agent_stream_writes_prompt_and_closes_stdin_with_parent_pipe_open() {
    use std::io::Read;
    use std::time::{Duration, Instant};

    let mut child = std::process::Command::new(current_test_binary())
        .arg("codex_stdin_probe_parent_child")
        .arg("--ignored")
        .arg("--nocapture")
        .env("NABU_CODEX_STDIN_PROBE_PARENT_CHILD", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let child_stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);

    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            drop(child_stdin);
            panic!("Codex stdin probe child timed out");
        }
        std::thread::sleep(Duration::from_millis(10));
    };

    drop(child_stdin);
    let mut stdout_text = String::new();
    let mut stderr_text = String::new();
    stdout.read_to_string(&mut stdout_text).unwrap();
    stderr.read_to_string(&mut stderr_text).unwrap();

    assert!(
        status.success(),
        "Codex stdin probe child failed with {status}\nstdout:\n{stdout_text}\nstderr:\n{stderr_text}"
    );
}

#[cfg(unix)]
#[ignore = "spawned by run_codex_agent_stream_writes_prompt_and_closes_stdin_with_parent_pipe_open"]
#[test]
fn codex_stdin_probe_parent_child() {
    if std::env::var_os("NABU_CODEX_STDIN_PROBE_PARENT_CHILD").is_none() {
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let vault = tempfile::tempdir().unwrap();
    let binary = executable_script(
        dir.path(),
        "codex",
        r#"stdin="$(cat)"
if [ "$stdin" != "Summarize" ]; then
  echo "unexpected stdin: $stdin" >&2
  exit 9
fi
printf '%s\n' '{"type":"thread.started","thread_id":"stdin-ok"}'
printf '%s\n' '{"type":"item.completed","item":{"id":"msg_1","type":"agent_message","text":"stdin prompt received"}}'
"#,
    );
    let mut events = Vec::new();
    let result = run_agent_stream_with_binary(
        &binary,
        codex_request(vault.path(), AiAgentPermissionMode::Safe),
        |event| events.push(event),
    );

    assert_eq!(result.unwrap(), "stdin-ok");
    assert!(events.iter().any(|event| matches!(
        event,
        AiAgentStreamEvent::TextDelta { text } if text == "stdin prompt received"
    )));
    assert!(matches!(events.last(), Some(AiAgentStreamEvent::Done)));
}

#[test]
fn codex_binary_candidates_include_supported_macos_installs() {
    let home = PathBuf::from("/Users/alex");
    let candidates = codex_binary_candidates_for_home(&home);
    let expected = [
        home.join(".local/bin/codex"),
        home.join(".codex/bin/codex"),
        home.join(".local/share/mise/shims/codex"),
        home.join(".asdf/shims/codex"),
        home.join(".npm-global/bin/codex"),
        home.join(".bun/bin/codex"),
        PathBuf::from("/Applications/Codex.app/Contents/Resources/codex"),
        PathBuf::from("/Applications/ChatGPT.app/Contents/Resources/codex"),
    ];

    for candidate in expected {
        assert!(
            candidates.contains(&candidate),
            "missing {}",
            candidate.display()
        );
    }
}

#[test]
fn codex_binary_candidates_include_linuxbrew_installs() {
    let home = PathBuf::from("/home/alex");
    let candidates = codex_binary_candidates_for_home(&home);
    let expected = [
        home.join(".linuxbrew/bin/codex"),
        PathBuf::from("/home/linuxbrew/.linuxbrew/bin/codex"),
    ];

    for candidate in expected {
        assert!(
            candidates.contains(&candidate),
            "missing {}",
            candidate.display()
        );
    }
}

#[test]
fn codex_binary_candidates_include_windows_npm_and_toolchain_shims() {
    let home = PathBuf::from("C:/Users/alex");
    let candidates = codex_binary_candidates_for_home(&home);
    let expected = [
        home.join(".local/bin/codex.exe"),
        home.join(".local/bin/codex.cmd"),
        home.join(".local/share/mise/shims/codex.exe"),
        home.join(".local/share/mise/shims/codex.cmd"),
        home.join(".asdf/shims/codex.exe"),
        home.join(".asdf/shims/codex.cmd"),
        home.join(".codex/bin/codex.cmd"),
        home.join(".npm-global/bin/codex.cmd"),
        home.join(".npm-global/bin/codex.exe"),
        home.join(".npm/bin/codex.cmd"),
        home.join(".npm/bin/codex.exe"),
        home.join(".bun/bin/codex.cmd"),
        home.join("AppData/Roaming/npm/codex.cmd"),
        home.join("AppData/Roaming/npm/codex.exe"),
        home.join("AppData/Local/pnpm/codex.cmd"),
        home.join("AppData/Local/pnpm/codex.exe"),
        home.join("scoop/shims/codex.cmd"),
        home.join("scoop/shims/codex.exe"),
    ];

    for candidate in expected {
        assert!(
            candidates.contains(&candidate),
            "missing {}",
            candidate.display()
        );
    }
}

#[test]
fn codex_availability_reports_installed_even_when_version_probe_fails() {
    let binary = PathBuf::from("C:/Users/alex/AppData/Roaming/npm/codex.cmd");

    let availability = codex_availability_from_binary_result(Ok(binary));

    assert!(availability.installed);
    assert_eq!(availability.version, None);
}

#[test]
fn codex_binary_candidates_include_nvm_managed_node_installs() {
    let home = tempfile::tempdir().unwrap();
    let codex = home.path().join(".nvm/versions/node/v22.12.0/bin/codex");
    std::fs::create_dir_all(codex.parent().unwrap()).unwrap();
    std::fs::write(&codex, "#!/bin/sh\n").unwrap();

    let candidates = codex_binary_candidates_for_home(home.path());

    assert!(candidates.contains(&codex), "missing {}", codex.display());
}

#[test]
fn first_existing_path_skips_empty_and_missing_lines() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("missing-codex");
    let codex = dir.path().join("codex");
    std::fs::write(&codex, "#!/bin/sh\n").unwrap();

    let stdout = format!("\n{}\n{}\n", missing.display(), codex.display());

    assert_eq!(first_existing_path(&stdout), Some(codex));
}

#[test]
fn windows_path_lookup_prefers_cmd_shim_over_extensionless_npm_script() {
    let dir = tempfile::tempdir().unwrap();
    let shell_script = dir.path().join("codex");
    let cmd_shim = dir.path().join("codex.cmd");
    std::fs::write(&shell_script, "#!/bin/sh\n").unwrap();
    std::fs::write(&cmd_shim, "@ECHO off\n").unwrap();

    let stdout = format!("{}\n{}\n", shell_script.display(), cmd_shim.display());

    assert_eq!(
        first_existing_path_for_platform(&stdout, true),
        Some(cmd_shim)
    );
}

#[cfg(unix)]
#[test]
fn command_path_from_shell_finds_codex_from_login_shell() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let codex = dir.path().join("codex");
    std::fs::write(&codex, "#!/bin/sh\n").unwrap();
    std::fs::set_permissions(&codex, std::fs::Permissions::from_mode(0o755)).unwrap();

    let shell = dir.path().join("shell");
    std::fs::write(
        &shell,
        format!(
            "#!/bin/sh\nif [ \"$1\" = \"-lc\" ]; then echo '{}'; fi\n",
            codex.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&shell, std::fs::Permissions::from_mode(0o755)).unwrap();

    assert_eq!(codex_path_from_shell(&shell), Some(codex));
}
