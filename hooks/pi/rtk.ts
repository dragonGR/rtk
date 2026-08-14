// RTK Pi extension — rewrites bash commands to use rtk for token savings.
// Requires: rtk >= 0.23.0 in PATH.
//
// This is a thin delegating extension: all rewrite logic lives in `rtk rewrite`,
// which is the single source of truth (src/discover/registry.rs).
// To add or change rewrite rules, edit the Rust registry — not this file.
//
// Exit code contract for `rtk rewrite`:
//   0 + stdout  Rewrite found → mutate command
//   1           No RTK equivalent → pass through unchanged
//   3 + stdout  Rewrite (advisory) → mutate command

// @ts-ignore
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent"
// @ts-ignore
import { isToolCallEventType } from "@earendil-works/pi-coding-agent"

declare const process: { env: Record<string, string | undefined> }

interface ToolCallEvent {
  input?: { command?: string }
  [key: string]: any
}

interface ToolCallContext {
  signal?: AbortSignal
}

const REWRITE_TIMEOUT_MS = 2_000
const MIN_SUPPORTED_RTK_MINOR = 23
const CACHE_CAPACITY = 100

// In-memory LRU cache to avoid subprocess spawn on repeated commands
const rewriteCache = new Map<string, string | null>()

function getCachedRewrite(cmd: string): string | null | undefined {
  if (!rewriteCache.has(cmd)) return undefined
  const val = rewriteCache.get(cmd)!
  rewriteCache.delete(cmd)
  rewriteCache.set(cmd, val)
  return val
}

function setCachedRewrite(cmd: string, val: string | null): void {
  if (rewriteCache.size >= CACHE_CAPACITY) {
    const firstKey = rewriteCache.keys().next().value
    if (firstKey !== undefined) {
      rewriteCache.delete(firstKey)
    }
  }
  rewriteCache.set(cmd, val)
}

const NON_ROUTABLE_PREFIXES = [
  "cd", "mkdir", "rmdir", "pwd", "export", "unset", "alias", "unalias",
  "echo", "printf", "exit", "clear", "touch", "cp", "mv", "chmod",
  "chown", "chgrp", "ln", "true", "false", "sleep", "which", "type"
]

function isNonRoutable(cmd: string): boolean {
  const trimmed = cmd.trim()
  if (trimmed === "" || trimmed.startsWith("rtk ")) return true
  const firstWord = trimmed.split(/\s+/)[0]
  return NON_ROUTABLE_PREFIXES.includes(firstWord)
}

// Calls `rtk rewrite`; returns the rewritten command or null (pass through).
async function rewriteCommand(
  pi: ExtensionAPI,
  cmd: string,
  signal?: AbortSignal
): Promise<string | null> {
  const cached = getCachedRewrite(cmd)
  if (cached !== undefined) {
    return cached
  }

  const result = await pi.exec("rtk", ["rewrite", cmd], {
    timeout: REWRITE_TIMEOUT_MS,
    signal,
  })

  if (result.killed || (result.code !== 0 && result.code !== 3)) {
    setCachedRewrite(cmd, null)
    return null
  }

  const rewritten = result.stdout.trim() || null
  setCachedRewrite(cmd, rewritten)
  return rewritten
}

export default async function (pi: ExtensionAPI) {
  // Probe rtk version at load time; disables extension if missing or too old.
  const ver = await pi.exec("rtk", ["--version"], { timeout: REWRITE_TIMEOUT_MS })
  if (ver.code !== 0) {
    console.warn("[rtk] rtk binary not found in PATH — extension disabled")
    return
  }

  // Warn and bail if rtk predates 0.23.0 (when `rtk rewrite` was introduced).
  const parsed = parseSemver(ver.stdout.replace(/^rtk\s+/, ""))
  if (parsed) {
    const [major, minor] = parsed
    if (major === 0 && minor < MIN_SUPPORTED_RTK_MINOR) {
      console.warn(`[rtk] rtk ${ver.stdout.trim()} is too old (need >= 0.23.0) — extension disabled`)
      return
    }
  }

  pi.on("tool_call", async (event: ToolCallEvent, ctx: ToolCallContext) => {
    try {
      if (typeof process !== "undefined" && process.env?.RTK_DISABLED === "1") return
      if (!isToolCallEventType("bash", event)) return

      const cmd = event.input?.command
      if (typeof cmd !== "string" || isNonRoutable(cmd)) return

      // Delegate to RTK.
      const rewritten = await rewriteCommand(pi, cmd, ctx.signal)
      if (rewritten && rewritten !== cmd) {
        if (event.input) {
          event.input.command = rewritten
        }
      }
    } catch (err) {
      // Fail open: never block execution on an unexpected error.
      console.warn("[rtk] unexpected error in tool_call handler; passing through command", err)
      return
    }
  })
}


// Parse "X.Y.Z" semver, return [major, minor, patch] or null.
function parseSemver(raw: string): [number, number, number] | null {
  const m = raw.trim().match(/(\d+)\.(\d+)\.(\d+)/)
  if (!m) return null
  return [parseInt(m[1], 10), parseInt(m[2], 10), parseInt(m[3], 10)]
}

