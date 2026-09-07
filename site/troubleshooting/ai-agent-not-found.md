# AI Agent Not Found

Nabu can only launch local CLI agents that are installed and discoverable.

## Symptoms

- The AI panel says no supported agent is available.
- Claude Code or another agent works in one shell but not in Nabu.

## Checks

Open a terminal and run the agent command directly. For Claude Code:

```bash
claude --version
```

If the command fails, install or repair the agent first.

## Path Issues

Desktop apps can inherit a different `PATH` from your interactive shell. Nabu checks common install locations, but shell setup can still vary. Prefer installing CLI tools in standard locations or making them available from your login shell.

