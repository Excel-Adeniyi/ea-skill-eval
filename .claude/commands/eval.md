---
description: Capture agent traces, assemble them, and score the results
argument-hint: "[--judge llm] [--platform claude_code] [--force]"
allowed-tools: Bash(python3 scripts/*), Bash(./scripts/run-eval.sh*), Bash(cargo run*), Bash(cargo test*), Read, Glob
---

Run the skill-evaluation pipeline for this repository.

Arguments passed by the user: `$ARGUMENTS`

## Steps

1. **Preview first.** Run `python3 scripts/capture.py --dry-run` with any
   `--platform` / `--case` / `--prompting` filters from `$ARGUMENTS`, and tell
   the user how many runs it will make. Each run costs real model usage.

2. **Capture.** Run `python3 scripts/capture.py` with the same filters. Report
   which captures succeeded, which failed, and — importantly — any that came
   back with **NO SENTINEL**, since that means the skill did not load and the
   run is not measuring what it appears to measure.

3. **Assemble.** Run `python3 scripts/build-traces.py`. Report the per-platform
   counts and anything it skipped.

4. **Evaluate.** Run `cargo run -- samples/captured.json` with `--judge llm` if
   the user asked for it, otherwise the heuristic. The LLM judge takes about 40
   seconds per uncached trace.

5. **Interpret, don't just paste.** After the tables, say:
   - which platform scored higher and on which metrics
   - whether the implicit trigger rate is lower than the explicit one
   - any row marked `(fallback)`, which means the judge was unreachable and
     those numbers are heuristic, not judge, scores
   - any score that looks wrong given the output — both scorers have known
     failure modes listed in README.md, and a surprising number is more often a
     scorer artefact than a real finding

## Rules

- If `$ARGUMENTS` is empty, default to the heuristic judge and **ask before
  capturing**, since a full run is 10 calls per platform.
- Never pass `--force` unless the user asked for it; it re-runs captures that
  already exist and spends usage again.
- If every capture lacks the sentinel, stop and report an installation problem
  rather than presenting scores.
