# RTK - Rust Token Killer (dragonGR Fork)

**Usage**: Token-optimized CLI proxy for shell commands (cuts 85% to 99% of bash output noise).

## Meta Commands

```bash
rtk gain              # Show token savings analytics
rtk gain --history    # Show command usage history with savings
rtk config            # View active configuration and settings
rtk proxy <cmd>       # Execute raw command without filtering (for debugging)
```

## Hook-Based Automatic Usage

Commands executed in terminal tools (`git`, `cargo`, `npm`, `pnpm`, `pytest`, `docker`, `kubectl`) are automatically rewritten by the shell hook.
Example: `git status` → `rtk git status` (transparent execution, zero overhead).

## AST Code Skeletonization

For inspecting source code files with 95%+ token savings:
```bash
rtk read <file>       # Extracts signatures, types, and structs; strips bodies
```

## Installation Verification

```bash
rtk --version         # Should show: rtk 0.45.0-dragonGR
rtk gain              # Verify analytics database
which rtk             # Verify correct binary location
```
