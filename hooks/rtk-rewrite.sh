#!/usr/bin/env bash
# rtk-hook-version: 2
# RTK Claude Code hook — rewrites commands to use rtk for token savings.
# Requires: rtk >= 0.23.0, jq
#
# This is a thin delegating hook: all rewrite logic lives in `rtk rewrite`,
# which is the single source of truth (src/discover/registry.rs).
# To add or change rewrite rules, edit the Rust registry — not this file.

if ! command -v jq &>/dev/null; then
  exit 0
fi

if ! command -v rtk &>/dev/null; then
  exit 0
fi

# Version guard: rtk rewrite was added in 0.23.0.
# Older binaries: warn once and exit cleanly (no silent failure).
RTK_VERSION=$(rtk --version 2>/dev/null | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1)
if [ -n "$RTK_VERSION" ]; then
  MAJOR=$(echo "$RTK_VERSION" | cut -d. -f1)
  MINOR=$(echo "$RTK_VERSION" | cut -d. -f2)
  # Require >= 0.23.0
  if [ "$MAJOR" -eq 0 ] && [ "$MINOR" -lt 23 ]; then
    echo "[rtk] WARNING: rtk $RTK_VERSION is too old (need >= 0.23.0). Upgrade: cargo install rtk" >&2
    exit 0
  fi
fi

INPUT=$(cat)
CMD=$(echo "$INPUT" | jq -r '.tool_input.command // empty')

if [ -z "$CMD" ]; then
  exit 0
fi

_matches_deny() {
  local cmd="$1"
  shift

  for settings_file in "$@"; do
    [ -f "$settings_file" ] || continue
    while IFS= read -r raw; do
      [ -z "$raw" ] && continue
      local inner="${raw#Bash(}"
      inner="${inner%)}"
      if [[ "$inner" == *":*" ]]; then
        local prefix="${inner%:*}"
        [[ "$cmd" == "$prefix" || "$cmd" == "$prefix "* ]] && return 0
      else
        [[ "$cmd" == "$inner" || "$cmd" == "$inner "* ]] && return 0
      fi
    done < <(jq -r '.permissions.deny[]? | select(startswith("Bash("))' "$settings_file" 2>/dev/null)
  done

  return 1
}

# Respect Claude permission deny rules before rewrite decision.
PROJECT_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || echo "")
if [ -z "$PROJECT_ROOT" ]; then
  _dir="$PWD"
  while [ "$_dir" != "/" ]; do
    [ -f "$_dir/.claude/settings.json" ] && { PROJECT_ROOT="$_dir"; break; }
    _dir=$(dirname "$_dir")
  done
fi

DENY_SOURCES=()
[ -n "$PROJECT_ROOT" ] && DENY_SOURCES+=("$PROJECT_ROOT/.claude/settings.json" "$PROJECT_ROOT/.claude/settings.local.json")
DENY_SOURCES+=("$HOME/.claude/settings.json" "$HOME/.claude/settings.local.json")

# Match deny patterns against first command segment.
FIRST_CMD=$(echo "$CMD" | sed -E 's/[[:space:]]*(&&|\|\||;|\|).*//' | sed -E 's/^[[:space:]]+|[[:space:]]+$//g')
if [ -n "$FIRST_CMD" ] && _matches_deny "$FIRST_CMD" "${DENY_SOURCES[@]}"; then
  exit 0
fi

# Delegate all rewrite logic to the Rust binary.
# rtk rewrite exits 1 when there's no rewrite — hook passes through silently.
REWRITTEN=$(rtk rewrite "$CMD" 2>/dev/null) || exit 0

# No change — nothing to do.
if [ "$CMD" = "$REWRITTEN" ]; then
  exit 0
fi

ORIGINAL_INPUT=$(echo "$INPUT" | jq -c '.tool_input')
UPDATED_INPUT=$(echo "$ORIGINAL_INPUT" | jq --arg cmd "$REWRITTEN" '.command = $cmd')

jq -n \
  --argjson updated "$UPDATED_INPUT" \
  '{
    "hookSpecificOutput": {
      "hookEventName": "PreToolUse",
      "permissionDecision": "allow",
      "permissionDecisionReason": "RTK auto-rewrite",
      "updatedInput": $updated
    }
  }'
