use super::*;

#[test]
fn build_codex_prompt_keeps_system_prompt_first() {
    let prompt = build_codex_prompt(&AgentStreamRequest {
        message: "Rename the note".into(),
        model: None,
        system_prompt: Some("Be concise".into()),
        vault_path: "/tmp/vault".into(),
        vault_paths: Vec::new(),
        permission_mode: AiAgentPermissionMode::Safe,
    });

    assert!(prompt.starts_with("System instructions:\nBe concise"));
    assert!(prompt.contains("User request:\nRename the note"));
}

#[test]
fn build_codex_args_uses_safe_default_permissions() {
    if let Ok(args) = build_codex_args(
        &AgentStreamRequest {
            message: "Rename the note".into(),
            model: None,
            system_prompt: None,
            vault_path: "/tmp/vault".into(),
            vault_paths: Vec::new(),
            permission_mode: AiAgentPermissionMode::Safe,
        },
        None,
    ) {
        assert_eq!(args[4], "exec");
        assert_codex_permission_contract(&args, AiAgentPermissionMode::Safe);
        assert!(args.contains(&"--json".to_string()));
        assert!(args.contains(&"-C".to_string()));
    }
}

#[test]
fn build_codex_args_passes_an_explicit_model_once() {
    let mut request = AgentStreamRequest {
        message: "Rename the note".into(),
        model: None,
        system_prompt: None,
        vault_path: "/tmp/vault".into(),
        vault_paths: Vec::new(),
        permission_mode: AiAgentPermissionMode::Safe,
    };
    request.model = Some("gpt-5.6-sol".into());

    let args = build_codex_args(&request, None).unwrap();
    let model_flags = args
        .windows(2)
        .filter(|window| window[0] == "--model" && window[1] == "gpt-5.6-sol")
        .count();

    assert_eq!(model_flags, 1);
}

#[test]
fn parses_only_visible_unique_models_from_debug_catalog() {
    let catalog = r#"{
        "models": [
            {"slug":"gpt-5.6-sol","display_name":"GPT-5.6 Sol","visibility":"list"},
            {"slug":"gpt-5.6-sol","display_name":"Duplicate","visibility":"list"},
            {"slug":"hidden","display_name":"Hidden","visibility":"hide"},
            {"slug":" ","display_name":"Invalid","visibility":"list"}
        ]
    }"#;

    assert_eq!(
        parse_codex_model_catalog(catalog).unwrap(),
        vec![CodexModelOption {
            id: "gpt-5.6-sol".into(),
            label: "GPT-5.6 Sol".into(),
        }]
    );
}

#[test]
fn malformed_model_catalog_returns_an_error() {
    assert!(parse_codex_model_catalog("not-json").is_err());
}

#[test]
fn codex_approval_policy_uses_only_supported_cli_values() {
    assert_eq!(
        codex_approval_policy(AiAgentPermissionMode::Safe),
        "on-request"
    );
    assert_eq!(
        codex_approval_policy(AiAgentPermissionMode::PowerUser),
        "never"
    );
}

#[test]
fn codex_power_user_keeps_workspace_write_without_dangerous_bypass() {
    if let Ok(args) = build_codex_args(
        &AgentStreamRequest {
            message: "Rename the note".into(),
            model: None,
            system_prompt: None,
            vault_path: "/tmp/vault".into(),
            vault_paths: Vec::new(),
            permission_mode: AiAgentPermissionMode::PowerUser,
        },
        None,
    ) {
        assert_codex_permission_contract(&args, AiAgentPermissionMode::PowerUser);
    }
}

#[test]
fn build_codex_args_can_request_last_message_output_file() {
    if let Ok(args) = build_codex_args(
        &AgentStreamRequest {
            message: "Rename the note".into(),
            model: None,
            system_prompt: None,
            vault_path: "/tmp/vault".into(),
            vault_paths: Vec::new(),
            permission_mode: AiAgentPermissionMode::Safe,
        },
        Some(Path::new("/tmp/nabu-codex-last-message.txt")),
    ) {
        assert!(args.windows(2).any(|window| window
            == [
                "--output-last-message",
                "/tmp/nabu-codex-last-message.txt",
            ]));
    }
}

#[test]
fn build_codex_args_uses_resolved_mcp_node_and_ui_bridge_env() {
    let args = build_codex_args(
        &AgentStreamRequest {
            message: "Read [[Test note]]".into(),
            model: None,
            system_prompt: None,
            vault_path: "/tmp/vault".into(),
            vault_paths: Vec::new(),
            permission_mode: AiAgentPermissionMode::Safe,
        },
        None,
    )
    .unwrap();

    let command_override = args
        .iter()
        .find(|arg| arg.starts_with("mcp_servers.nabu.command="))
        .expect("Codex should receive a transient Nabu MCP command");

    assert!(
        !command_override.ends_with(r#""node""#),
        "Codex MCP command should use Nabu's resolved Node path, got {command_override}"
    );
    assert!(
        command_override.contains('/'),
        "Codex MCP command should be an absolute Node path, got {command_override}"
    );
    assert!(args.iter().any(|arg| arg.contains(r#"WS_UI_PORT="9711""#)));
}

#[test]
fn build_codex_command_keeps_agent_process_contract() {
    let binary = PathBuf::from("codex");
    let args = vec!["exec".to_string(), "--json".to_string()];
    let command = build_codex_command(&binary, args, "/tmp/vault").unwrap();
    let actual_args: Vec<&OsStr> = command.get_args().collect();

    assert_eq!(command.get_program(), OsStr::new("codex"));
    assert_eq!(
        actual_args,
        vec![OsStr::new("exec"), OsStr::new("--json"), OsStr::new("-")]
    );
    assert!(!actual_args.contains(&OsStr::new("Summarize")));
    assert_eq!(command.get_current_dir(), Some(Path::new("/tmp/vault")));
}

#[test]
fn build_codex_command_extends_path_with_resolved_homebrew_bin() {
    let binary = PathBuf::from("/opt/homebrew/bin/codex");
    let command = build_codex_command(
        &binary,
        vec!["exec".to_string(), "--json".to_string()],
        "/tmp/vault",
    )
    .unwrap();
    let path_value = command
        .get_envs()
        .find(|(key, _)| *key == OsStr::new("PATH"))
        .and_then(|(_, value)| value)
        .expect("PATH should be set");
    let paths = std::env::split_paths(path_value).collect::<Vec<_>>();

    assert!(
        paths.contains(&PathBuf::from("/opt/homebrew/bin")),
        "PATH should include the resolved Codex binary directory, got {paths:?}"
    );
}

#[test]
fn build_codex_command_avoids_windows_cmd_shim_for_complex_args() {
    let dir = tempfile::tempdir().unwrap();
    let shim = dir.path().join("codex.cmd");
    let script = dir
        .path()
        .join("node_modules")
        .join("@openai")
        .join("codex")
        .join("bin")
        .join("codex.js");
    std::fs::create_dir_all(script.parent().unwrap()).unwrap();
    std::fs::write(&script, "console.log('codex')\n").unwrap();
    std::fs::write(
        &shim,
        r#"@ECHO off
"%_prog%" "%dp0%\node_modules\@openai\codex\bin\codex.js" %*
"#,
    )
    .unwrap();

    let command = build_codex_command(
        &shim,
        vec![
            "exec".to_string(),
            "-c".to_string(),
            r#"mcp_servers.nabu.command="C:\\Program Files\\node.exe""#.to_string(),
        ],
        "/tmp/vault",
    )
    .unwrap();

    assert_ne!(
        command.get_program(),
        shim.as_os_str(),
        "Codex npm .cmd shims cannot safely receive quoted -c args directly"
    );
    let actual_args = command.get_args().collect::<Vec<_>>();
    assert_eq!(actual_args.first().copied(), Some(script.as_os_str()));
    assert!(actual_args.iter().any(|arg| *arg == OsStr::new("-")));
    assert!(!actual_args
        .iter()
        .any(|arg| *arg == OsStr::new("Summarize")));
}
