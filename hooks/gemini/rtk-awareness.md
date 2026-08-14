# RTK - Rust Token Killer (Google Gemini CLI / Antigravity)

> [!IMPORTANT]
> **CRITICAL RULE**: ALWAYS prefix terminal and shell execution commands with `rtk`.
> RTK is a high-performance local proxy that cuts 85%–99% of bash output noise, preserving context memory and preventing rate limits.

## 1. MANDATORY COMMAND REWRITING TABLE

Whenever you execute shell or terminal commands, format them with `rtk`:

| Standard Command | MANDATORY RTK Format |
|---|---|
| `git status` / `git log` / `git diff` | `rtk git status` / `rtk git log` / `rtk git diff` |
| `cargo test` / `cargo build` | `rtk cargo test` / `rtk cargo build` |
| `npm test` / `pnpm typecheck` / `yarn` | `rtk npm test` / `rtk pnpm typecheck` |
| `pytest` / `python -m pytest` | `rtk pytest` |
| `cat <file>` / file inspection | `rtk read <file>` *(AST Skeletonization Mode)* |
| `grep` / `rg` / `find` | `rtk grep` / `rtk rg` / `rtk find` |
| `docker` / `kubectl` / `aws` | `rtk docker` / `rtk kubectl` / `rtk aws` |

## 2. DIRECTIVES FOR GEMINI CLI & ANTIGRAVITY

1. **Automatic Hook Interception**: In Gemini CLI, the shell hook in `~/.gemini/settings.json` automatically rewrites commands transparently.
2. **AST Code Skeletonization**: When inspecting source files, use `rtk read <file>` to extract function signatures, structs, and types while stripping bodies (95%+ token savings).
3. **Analytics & Config**: Run `rtk gain` to view token savings analytics or `rtk config` to view active settings.
4. **Bypass Debugging**: Use `rtk proxy <cmd>` if raw unmodified command output is required.
