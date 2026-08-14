# LLM Agent Hooks (dragonGR Fork)

## Scope

**Deployed hook artifacts** — the actual files installed on user machines by `rtk init`. These are shell scripts, TypeScript plugins, Rust binary hooks, and rules files that run outside or alongside the Rust binary. They act as **thin delegates**: parse agent-specific JSON, call `rtk rewrite` as a subprocess, and format the response in the target agent's expected structure with zero overhead.

Owns: per-agent hook scripts and configuration files for supported agents (Claude Code, Gemini CLI, Google Antigravity, Copilot, Cursor, Cline, Windsurf, Codex, OpenCode, Hermes, Pi, Mistral Vibe, Factory Droid, KiloCode, Kimi).

Does **not** own: hook installation/uninstallation (that's `src/hooks/init.rs`), the rewrite pattern registry (that's `src/discover/registry.rs`), or integrity verification (that's `src/hooks/integrity.rs`).

---

## Global-First Installation Model (20+ Developer Projects)

For developers managing dozens of projects, RTK recommends **Global User-Level Activation**. You run activation **once** on your machine, and RTK optimizes terminal commands across **all 20+ repositories** automatically without cluttering project folders:

```bash
rtk init -g                  # Global hook for Claude Code (~/.claude/settings.json)
rtk init -g --gemini         # Global hook for Gemini CLI (~/.gemini/settings.json)
rtk init --agent pi          # Global plugin for Pi Agent (~/.pi/)
rtk init --agent antigravity # Global/Project rules for Google Antigravity
rtk init --codex             # Global rules for OpenAI Codex CLI (AGENTS.md)
rtk init --show              # View active global hook status
```

---

## Purpose

LLM agent integrations intercept CLI commands and route them through RTK for token optimization. Each hook transparently rewrites raw commands (e.g., `git status`) to their RTK equivalents (e.g., `rtk git status`), cutting **85% to 99%** of output noise without requiring the agent or user to manually type prefixes.

---

## How It Works

```
Agent runs command (e.g., "cargo test --nocapture")
  -> Hook intercepts (PreToolUse / plugin event)
  -> Reads JSON input, extracts command string
  -> Calls `rtk rewrite "cargo test --nocapture"`
  -> Registry matches pattern, returns "rtk cargo test --nocapture"
  -> Hook sends response in agent-specific JSON format
  -> Agent executes "rtk cargo test --nocapture" instead
  -> Filtered output reaches LLM (85%-99% fewer bash output tokens)
```

---

## Supported Agents Matrix

| Agent | Mechanism | Hook Type | Modifies Command? | Location / Target |
|-------|-----------|-----------|-------------------|-------------------|
| **Claude Code** | Shell hook (`PreToolUse`) | Transparent rewrite | Yes (`updatedInput`) | `~/.claude/hooks/` & `~/.claude.json` |
| **Gemini CLI** | Rust binary (`rtk hook gemini`) | Transparent rewrite | Yes (`hookSpecificOutput`) | `~/.gemini/hooks/` & `~/.gemini/settings.json` |
| **Google Antigravity** | Rules file | Prompt-level guidance | Yes (via prompt rules) | `.agents/rules/antigravity-rtk-rules.md` |
| **VS Code Copilot Chat** | Rust binary (`rtk hook copilot`) | Transparent rewrite | Yes (`updatedInput`) | VS Code Extension settings |
| **GitHub Copilot CLI** | Rust binary (`rtk hook copilot`) | Deny-with-suggestion | No (agent retries) | `~/.copilot-cli/` |
| **Cursor** | Rust binary | Transparent rewrite | Yes (`updated_input`) | `.cursorrules` / `~/.cursor/` |
| **Cline / Roo Code** | Rules file | Prompt-level guidance | N/A | `.clinerules` |
| **Windsurf** | Rules file | Prompt-level guidance | N/A | `.windsurfrules` |
| **Codex CLI** | Rules file (`AGENTS.md`) | Prompt-level guidance | N/A | `AGENTS.md` / `~/.codex/` |
| **OpenCode** | TypeScript plugin (`tool.execute.before`) | In-place mutation | Yes | `zx` plugin |
| **Pi Agent** | TypeScript extension (`tool_call` event) | In-place mutation | Yes | `~/.pi/rtk.ts` |
| **Hermes CLI** | Python plugin (`pre_tool_call`) | In-place mutation | Yes | `~/.hermes/hooks/` |
| **Mistral Vibe** | Rust binary (`rtk hook vibe`) | Transparent rewrite | Yes | `~/.vibe/hooks.toml` |
| **Factory Droid** | Plugin wrapper | Transparent rewrite | Yes | `~/.droid/` |

---

## Optimization Engine Features

| Category | Optimization Mechanism | Typical Savings |
|----------|------------------------|-----------------|
| **AST Code Skeletonization** | Extracts structs, types, function signatures; strips bodies (`rtk read`) | **95% – 97%** |
| **Workspace Delta Caching** | Suppresses repeated command output; emits concise diffs (`rtk --delta`) | **99%** |
| **Stack Trace Pruning** | Strips internal framework frames (`pytest`, `cargo`, `jest`, `vitest`) | **95%** |
| **API & K8s Payload Trimming** | Removes `managedFields`, `ownerReferences`, `resourceVersion` (`kubectl`, `gh api`) | **90%** |
| **SIMD Head/Tail Truncation** | Preserves top 10 lines + bottom 15 lines for unparsed stream fallbacks | **90% – 95%** |
| **Ultra-Compact Mode** | Strips ASCII borders and padding (`ultra_compact = true`) | **15% – 20% extra** |

---

## JSON Formats by Agent

### 1. Claude Code (Shell Hook)
**Input** (stdin):
```json
{
  "tool_name": "Bash",
  "tool_input": { "command": "git status" }
}
```
**Output**:
```json
{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "allow",
    "permissionDecisionReason": "RTK auto-rewrite",
    "updatedInput": { "command": "rtk git status" }
  }
}
```

### 2. Gemini CLI (Rust Binary)
**Input** (stdin):
```json
{
  "tool_name": "run_shell_command",
  "tool_input": { "command": "git status" }
}
```
**Output**:
```json
{
  "decision": "allow",
  "hookSpecificOutput": {
    "tool_input": { "command": "rtk git status" }
  }
}
```

### 3. Cursor (Shell Hook / Binary)
**Output**:
```json
{
  "permission": "allow",
  "updated_input": { "command": "rtk git status" }
}
```

---

## Exit Code Contract & Graceful Degradation

Hooks are **non-blocking**: an error in RTK or an unparsed JSON input must **never** prevent a user command from running:

- Missing binary or hook script $\rightarrow$ Exit 0 (command runs raw).
- Invalid JSON input $\rightarrow$ Pass through unchanged, exit 0.
- `rtk rewrite` subprocess error $\rightarrow$ Exit 0.
- Filter logic fallback $\rightarrow$ Return raw output.

---

## Installation & Verification Commands

```bash
rtk init -g                  # Global hook for Claude Code
rtk init -g --gemini         # Global hook for Gemini CLI
rtk init --agent antigravity # Rules & prompt hook for Google Antigravity
rtk init --agent pi          # Extension hook for Pi Agent
rtk init --codex             # Rules hook for OpenAI Codex CLI
rtk init --show              # Display active agent hook status
rtk init -g --uninstall     # Clean uninstall of RTK artifacts
```
