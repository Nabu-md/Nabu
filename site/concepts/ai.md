# AI

Nabu has two AI paths: coding agents that can use tools to inspect and edit a vault, and direct model targets that answer in chat mode from note context.

## Coding Agents

The AI panel can stream supported local CLI agents through Nabu's normalized event layer. Current targets include:

- Claude Code
- Codex
- GitHub Copilot
- OpenCode
- Pi
- Antigravity CLI
- Kiro
- Hermes Agent

Nabu detects agents installed on the machine. Each agent still owns its authentication, available tools, and runtime behavior.

Coding agents can run in:

- **Vault Safe** mode, limited to file, search, and edit tools.
- **Power User** mode, which can allow local shell commands scoped to the active vault for agents that support shell access.

## Agent Models

Agents that expose a reliable model catalog can show a model selector in the AI workspace. Nabu currently discovers Codex models and exposes Claude Code's documented aliases. Other agents continue to use their own default model.

Model preferences are stored per agent on the current device. Switching agents restores that agent's previous choice, and selecting **Agent default** lets the CLI decide.

## Direct Models

Direct model targets run in chat mode. They receive the active note, linked context, and conversation history, but they do not receive vault-write tools or shell access.

Supported provider shapes include:

- Local models through Ollama or LM Studio.
- Hosted providers such as OpenAI, Anthropic, Gemini, and OpenRouter.
- Custom OpenAI-compatible endpoints.

## External MCP Setup

Nabu exposes an MCP server for external tools. The setup flow can write Nabu's MCP entry into Claude Code, Antigravity CLI, Cursor, and a generic MCP config path, and it can also copy the exact JSON snippet for manual setup.

MCP setup is explicit. Closing the dialog leaves third-party config files untouched.

The MCP server can search and read vault content, create notes, update a complete note, or append content to an existing note. `update_note` supports an optional modification-time guard for safer read-modify-write workflows, and both write operations refresh Nabu after the file changes.

## Why Git Matters For AI

AI-generated changes should be inspectable. Git gives you diffs, history, rollback, and a clear boundary between suggestions and committed work.
