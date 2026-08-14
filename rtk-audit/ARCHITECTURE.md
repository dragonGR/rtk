# RTK Architecture & System Design

## Overview

RTK (Rust Token Killer) is a high-performance CLI proxy designed to intercept tool commands issued by LLM coding agents (e.g., Pi Agent, Claude Code, Cursor, Codex) and strip low-signal boilerplate, ansi codes, redundant progress banners, and duplicate outputs before returning data to the model context.

---

## Core Principles

1. **Zero-Allocation Fast-Path Processing**:
   - Strip ANSI escape sequences using single-pass byte scanning over `&[u8]`.
   - Short-circuit non-routable shell built-ins (`cd`, `pwd`, `mkdir`, `export`, `echo`, `touch`, `cp`, `mv`) in agent extensions before spawning subprocesses.
   - Use `memchr` SIMD-accelerated byte searching for line boundary detection and output windowing.

2. **Strict Memory & Resource Capping**:
   - Enforce a 16MB strict limit on file and stdin reads (`read_text_file_capped`, `read_text_stdin_capped`) to prevent OOM panics on huge outputs.

3. **Subprocess & Execution Efficiency**:
   - Run single-pass execution for compound command operations (e.g. `git diff --patch-with-stat`, single-pass `git show`, single-execution `docker ps` and `docker images`).
   - Use $O(1)$ HashMap lookups for log level deduplication.

---

## Subsystem Layout

- `src/main.rs`: CLI argument parsing via `clap` and main entry point routing.
- `src/core/`:
  - `config.rs`: Configuration file parsing (`~/.config/rtk/config.toml`).
  - `tracking.rs`: SQLite persistence for local savings metrics with throttled DB cleanup.
  - `filter.rs`: Core filtering strategies (`MinimalFilter`, `AggressiveFilter`, `smart_truncate`).
  - `stream.rs`: Lossy UTF-8 streaming filters and block line handlers.
  - `utils.rs`: Fast single-pass `strip_ansi` byte scanner and capped text readers.
  - `truncate.rs`: SIMD-accelerated line counting (`count_lines_simd`) and windowing (`truncate_lines_simd`).
- `src/discover/`:
  - `registry.rs`: Fast command classification and rewrite rule evaluation.
- `src/cmds/`:
  - Specialized CLI filter wrappers (`git`, `rust`, `js`, `python`, `cloud`, `system`).
- `hooks/`:
  - Extension integrations for AI coding agents (`pi`, `claude`, `cursor`, etc.).
