---
description: Fast offline regression check — no model calls, no usage spent
allowed-tools: Bash(cargo run*), Bash(cargo test*), Bash(cargo clippy*), Bash(cargo fmt*)
---

Run the offline checks for this repository. **Make no model calls** — this
command must not spend usage.

1. `cargo test` — all unit and integration tests.
2. `cargo clippy --all-targets` — must be warning-free.
3. `cargo fmt --check` — must be clean.
4. If `samples/captured.json` exists, run
   `cargo run -- samples/captured.json --threshold 0.5` and report the exit
   code. Exit 2 means at least one trace scored below the threshold; name which.

Report pass/fail for each step. If anything fails, show the actual error output
rather than summarising it.
