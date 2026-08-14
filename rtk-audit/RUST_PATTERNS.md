# RTK Rust Development & Code Standards

## Code Standards

1. **Zero AI Slop**:
   - Write concise, explicit technical Rust code.
   - Avoid placeholder comments (`// TODO`, `/* logic here */`).
   - Keep function signatures explicit and fully typed.

2. **Error Handling**:
   - Use `anyhow::{Context, Result}` for error context in CLI commands.
   - Explicitly handle failure modes instead of swallowing errors or panicking.

3. **Performance Conventions**:
   - Prefer byte-slice scanning over regex string allocations where feasible.
   - Use `OnceLock` / `LazyLock` for static indices and compiled patterns.
   - Use `memchr` SIMD iteration for line boundary scanning in large outputs.
   - Enforce `TEXT_CAPTURE_MAX_BYTES` (16MB) on file/stdin buffer reads.
