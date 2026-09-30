# Runbook: capturing and evaluating real traces

End-to-end steps for testing the portable skill across agent platforms.

## 1. Install the skill

**Claude Code** — the skill lives at `skills/trace-evaluator/SKILL.md`. Either
point Claude Code at this repository, or copy it into the skills directory:

```bash
mkdir -p ~/.claude/skills/trace-evaluator
cp skills/trace-evaluator/SKILL.md ~/.claude/skills/trace-evaluator/
```

**Codex CLI** — `AGENTS.md` at the repository root points at the same file.
Run Codex from this directory so it picks that up.

**Qwen Code** — check which context file your build reads and add a pointer to
`skills/trace-evaluator/SKILL.md`, mirroring `AGENTS.md`.

## 2. Smoke-test before capturing in bulk

Do **one** run on each platform before committing to the full set. Paste the
first explicit prompt from `samples/cases/PROMPTS.md` and confirm the response
begins with:

```
<!-- skill: trace-evaluator v1 -->
```

If the sentinel is missing, the skill did not load, and every capture after that
records `skill_triggered: false`. Fix the install before continuing — this is
the five-minute check that saves an hour of useless captures.

## 3. Automated capture (recommended)

Both `claude` and `codex` run non-interactively, so the capture loop can be
driven end to end — no copy-paste:

```bash
# See what would run, without spending anything
python3 scripts/capture.py --dry-run

# Capture everything, then assemble and evaluate in one step
./scripts/run-eval.sh --judge llm

# Or narrow it down
python3 scripts/capture.py --platform claude_code --prompting explicit
python3 scripts/capture.py --case case-3-no-format-requested
```

`capture.py` inlines the rubric into each prompt rather than relying on the
platform to auto-load it, because a run that silently used no skill at all is
worse than no run. The explicit/implicit distinction is preserved in the
*request* wording, which is the variable under test.

Existing captures are skipped unless you pass `--force`, so an interrupted run
resumes where it stopped. Each line reports whether the sentinel came back.

**Each run costs real model usage.** Ten runs per platform. Start with
`--dry-run`.

### Missing sentinel stops the run

If a response comes back without the sentinel marker, capture **stops with exit
code 2** and tells you which file to inspect. A skill that did not load means
the run measures the model's general behaviour rather than the rubric, and
continuing would spend usage on data that answers a different question.

For implicit runs, where a skill legitimately may not fire, that non-firing *is*
the result you are measuring:

```bash
python3 scripts/capture.py --prompting implicit --allow-missing-sentinel
```

Keep it strict for explicit runs — there, a missing sentinel is always an
installation problem, never a finding.

### Manual capture

If a platform has no scriptable mode, fall back to pasting from
`samples/cases/PROMPTS.md` by hand — see steps 4 and 5.

## 3b. Run the cases manually

`samples/cases/PROMPTS.md` has the exact text to paste: five cases × two
prompting modes (explicit names the skill, implicit does not) per platform.

Each case targets one rule in `SKILL.md`, so a low score tells you *which*
instruction was dropped:

| Case | Rule it tests |
|---|---|
| `case-1-baseline` | Control — everything correct |
| `case-2-format-content-split` | Score metrics independently |
| `case-3-no-format-requested` | FormatCompliance is 1.0 when no format was asked for |
| `case-4-meta-instruction-trap` | "Reply as JSON" is format, not a recall item |
| `case-5-total-miss` | Use the full range — a total miss is 0.0 |

## 4. Save each response

One file per run, in `samples/cases/raw/`, named:

```
<platform>-<prompting>-<case-id>.txt
```

Platforms: `claude_code`, `codex_cli`, `qwen_code`.
Prompting: `explicit`, `implicit`.

For example:

```
samples/cases/raw/claude_code-explicit-case-2-format-content-split.txt
samples/cases/raw/codex_cli-implicit-case-5-total-miss.txt
```

Paste the response verbatim, sentinel included. Do not tidy it — formatting
failures are data.

## 5. Assemble the trace file

```bash
python3 scripts/build-traces.py
```

Writes `samples/captured.json`, reporting how many traces came from each
platform and skipping any file it cannot match (with the reason).

The script scores captures against **what `SKILL.md` demanded** — emit the
sentinel, produce five named metrics, reply with only JSON — not against the
instructions inside the trace being evaluated. The platform's job was to follow
the rubric, so that is what the metrics measure. Edit `MODELS` at the top of the
script to record the exact model each platform ran.

## 6. Evaluate

```bash
# Fast offline baseline
cargo run -- samples/captured.json

# Model judge; first run ~40s per trace, re-runs are cached
cargo run -- samples/captured.json --judge llm

# Machine-readable, for keeping alongside the writeup
cargo run -- samples/captured.json --judge llm --format json > results.json
```

Read the **platform comparison** table for the side-by-side, and the **skill
triggering** table for the implicit-prompting result — the interesting number is
whether the implicit rate is lower than the explicit one.

## 7. Read the two scorers against each other

Run both and compare:

```bash
cargo run -- samples/captured.json --format json > heuristic.json
cargo run -- samples/captured.json --judge llm --format json > judge.json
```

Where the heuristic and the judge agree on the ranking, the result is credible.
Where they diverge, read those traces by hand — one of the two scorers is wrong,
and which one is the finding.

Both scorers have known failure modes; see the Limitations section of
[README.md](README.md) before drawing conclusions.

## Automation

**Slash commands** (Claude Code):

- `/eval` — capture, assemble, score, and interpret the tables. Asks before
  spending usage.
- `/eval-check` — offline only: tests, clippy, fmt, threshold gate. No model
  calls.

**CI** (`.github/workflows/ci.yml`) runs on every push and pull request:
`cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, a CLI smoke
test asserting valid JSON with no fallbacks, and a syntax check on the capture
scripts. It makes **no model calls**, so it is free and deterministic.

The threshold gate activates only when `samples/captured.json` is present.
Captured traces are gitignored by default; commit a set deliberately if you want
CI to guard against score regressions.

## Troubleshooting

**Every trace says `skill_triggered: false`** — the sentinel is missing. The
skill is not loading. Go back to step 2.

**Rows labelled `(fallback)`** — the judge was unreachable and the heuristic ran
instead. Check Ollama is up: `ollama list`.

**`--judge llm` is slow the first time** — expected, roughly 40s per trace
locally. Re-runs are served from `.skill-eval-cache/` in milliseconds. Editing a
trace re-judges only that trace.

**Scores changed but the traces did not** — the cache keys on trace content,
judge name and prompt version. If you edited `SKILL.md`'s rubric, bump
`PROMPT_VERSION` in `src/judge/prompt.rs` so stale entries are not reused.
