# RTK Testing Standards & Verification Guidelines

## Overview

RTK enforces strict verification requirements. Every new feature, filter modification, or bug fix must be covered by unit tests and integration test suites.

---

## Test Execution

Before presenting changes or submitting pull requests, run the following verification commands:

```bash
# Verify clean compilation without compiler warnings
cargo check

# Run all unit tests and integration test suites
cargo test
```

`-D warnings` is strictly enforced across all crates.

---

## Testing Patterns

### 1. Snapshot Testing with `insta`

RTK uses `insta` for snapshot-based filter output verification.

```rust
use insta::assert_snapshot;

#[test]
fn test_git_log_output() {
    let input = include_str!("../tests/fixtures/git_log_raw.txt");
    let output = filter_git_log(input);

    assert_snapshot!(output);
}
```

### 2. Integration Test Suites (`tests/`)

Integration tests run real CLI binary invocations across simulated process outputs:
- `copilot_selfheal_test.rs`: Config self-healing and preservation invariant checks.
- `grep_context_test.rs` & `grep_faithful_format_test.rs`: Output formatting fidelity.
- `guard_integration_test.rs`: Filter passthrough vs compression guard checks.
- `search_compress_test.rs` & `search_faithful_test.rs`: `rg` and `grep` token savings.
