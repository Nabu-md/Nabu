# Model Provider Connection

Source: troubleshooting/model-provider-connection.md
URL: /troubleshooting/model-provider-connection

# Model Provider Connection

Use this checklist when a local or API model provider does not connect.

## Local Providers

For Ollama or LM Studio:

1. Start the local model server.
2. Confirm the base URL in Nabu matches the server.
3. Confirm the model ID is installed and loaded by the provider.
4. Use the Settings test action again.

## API Providers

For hosted providers:

1. Confirm the provider kind and endpoint.
2. Confirm the model ID exists for your account.
3. Confirm the API key is saved locally or available in the configured environment variable.
4. Avoid storing secrets in the vault.

## Chat Mode Boundary

Direct model targets run in chat mode. If you need file-editing tools, use a coding agent target such as Claude Code, Codex, OpenCode, Pi, or Antigravity CLI.