# ea-skill-eval

A command-line harness for evaluating agent traces on five instruction-following
metrics, with two independent scorers: fast offline heuristics, and any
OpenAI-compatible model acting as a judge.

Built to compare how the same portable skill behaves across different agent
platforms — Claude Code, Codex CLI, Qwen Code — by scoring the traces each one
produces.

## Try it in 30 seconds

Needs only Rust. No API keys, no model downloads, no Python.

```bash
git clone <this repo> && cd skill-eval
cargo run -- samples/captured.json
```

That scores 20 real captured traces — 10 from Claude Code, 10 from Codex CLI —
and prints the comparison tables. Everything in that command runs offline.

### What you need for the rest

| Feature | Requires |
|---|---|
| Heuristic scoring, tests, CI | Rust only |
| Capture your own traces | Python 3, plus `claude` and/or `codex` on PATH |
| LLM judge (`--judge llm`) | [Ollama](https://ollama.com) + `ollama pull qwen3.6` |
| Live chat scoring (Stop hook) | Claude Code, plus `codex` on PATH |

Nothing beyond the first row is needed to see the project work.

## Quick start

```bash
# Offline heuristics against the bundled samples. Instant.
cargo run

# Judge with a local model through Ollama.
cargo run -- --judge llm --model qwen3.6:latest

# Machine-readable output for CI.
cargo run -- --format json

# Re-runs of an unchanged trace file are served from cache: 215s -> 0.01s.
cargo run -- --judge llm --clear-cache

# Gate a build: exit 2 if any trace's mean falls below 0.5.
cargo run -- --threshold 0.5
```

## Usage

```
ea-skill-eval [INPUT] [OPTIONS]

Arguments:
  [INPUT]  JSON file of captured traces [default: samples/traces.json]

Options:
      --judge <JUDGE>        heuristic | llm [default: heuristic]
      --model <MODEL>        Model for --judge llm [default: qwen3.6:latest]
      --base-url <BASE_URL>  OpenAI-compatible endpoint
                             [default: http://localhost:11434/v1]
      --api-key <API_KEY>    Bearer token [env: SKILL_EVAL_API_KEY]
      --format <FORMAT>      table | json [default: table]
      --threshold <FLOAT>    Exit 2 if any trace scores below this
      --metric <METRIC>      adherence | relevancy | precision | recall | format
                             Repeatable; defaults to all five
      --cache-dir <DIR>      Judged-result cache [default: .skill-eval-cache]
      --no-cache             Judge every trace afresh
      --clear-cache          Delete cached results, then exit
```

### Caching

Judged results are cached on disk, keyed on a hash of the trace's
instructions/task/output plus the judge name and prompt version. Editing a
trace, switching models or changing the rubric all invalidate the entry, so a
stale score is never served for text that has changed.

Measured on the bundled samples with a local qwen3.6: **214.78s cold, 0.01s
warm.** Only the LLM judge is cached; the heuristic recomputes faster than a
disk read.

### Exit codes

| Code | Meaning                                    |
|------|--------------------------------------------|
| 0    | Success, or all traces cleared `--threshold` |
| 1    | Input could not be loaded or parsed        |
| 2    | One or more traces fell below `--threshold` |

## Trace format

```json
[
  {
    "id": "trace-001",
    "instructions": "Reply as JSON.",
    "task": "Give the capital of France.",
    "output": "Sure! The capital of France is Paris.",

    "platform": "claude_code",
    "model": "claude-opus-5",
    "prompting": "explicit",
    "skill_triggered": true
  }
]
```

The first four fields are required. The last four are optional and default to
`unknown` / `null` / `explicit` / `null`, so trace files captured before those
fields existed still load.

- **`platform`** — `claude_code`, `codex_cli`, `qwen_code` or `unknown`.
- **`prompting`** — `explicit` if the prompt named the skill, `implicit` if it
  described only the task and left the agent to decide.
- **`skill_triggered`** — whether the skill actually ran. Separate from
  `prompting` on purpose: the interesting question is whether a skill still
  fires under *implicit* prompting.

## Capturing real traces

`claude` and `codex` both run non-interactively, so capture is automated:

```bash
./scripts/run-eval.sh --judge llm      # capture -> assemble -> evaluate
python3 scripts/capture.py --dry-run   # preview without spending usage
```

See [RUNBOOK.md](RUNBOOK.md) for the full workflow, including manual capture for
platforms without a scriptable mode.

## The portable skill

The rubric lives in one file, [`skills/trace-evaluator/SKILL.md`](skills/trace-evaluator/SKILL.md),
with YAML frontmatter for Claude Code. [`AGENTS.md`](AGENTS.md) at the repository
root points Codex CLI and similar surfaces at that same file rather than
duplicating it.

That indirection is the point: if two platforms read different rubrics, any
difference in their scores is an artefact of the instructions, not a finding
about the models.

### The sentinel marker

The skill instructs agents to begin every response with:

```
<!-- skill: trace-evaluator v1 -->
```

Most agent surfaces give no machine-readable signal that an instruction file was
read, and self-reported "I used the skill" is not evidence. The marker is
checkable after the fact.

On load, a trace with no explicit `skill_triggered` value has it inferred from
whether this marker is present; an explicit value in the file always wins. The
marker is then stripped from the output so its own words cannot inflate
term-overlap scores.

This matters most for implicit prompting, where the question is whether the
skill fires *without* being named — which has no answer without an observable
marker.

## The five metrics

| Metric                 | Question it answers                                     |
|------------------------|---------------------------------------------------------|
| Instruction Adherence  | Did the output do what it was told?                     |
| Task Relevancy         | Did the output address the task?                        |
| Instruction Precision  | Was the output free of content nobody asked for?        |
| Instruction Recall     | Did the output cover everything that was asked?         |
| Format Compliance      | Did the output obey stated formatting requirements?     |

A metric that has nothing to measure is **skipped**, not scored zero. A trace
whose instructions state no format requirement has not *failed* format
compliance, and reporting `0.00` there would be a lie. Skipped metrics render as
`-` in tables and `null` in JSON.

## The two scorers

**Heuristic** (`--judge heuristic`) compares stemmed term sets between the
instructions, task and output. Offline, deterministic, instant. It measures word
overlap, not meaning.

**LLM judge** (`--judge llm`) sends one request per trace and asks for all five
metrics in a single response. Any OpenAI-compatible endpoint works — local
Ollama, Zhipu's GLM endpoint, OpenAI itself — so switching judges is
configuration, not code.

If the judge fails, the heuristic runs instead and the row is labelled
`(fallback)`. Fallback numbers are never presented as judge numbers.

## Limitations

**The heuristic is a weak baseline.** It counts word overlap. It penalises
instruction terms that describe *how* to answer rather than what to say — an
output that correctly replies in JSON still loses recall points for not
containing the literal word "JSON", because `FormatCompliance` already covers
that dimension and recall double-counts it. Treat heuristic scores as a
platform-neutral control, not as ground truth.

**The judge is not ground truth either.** On the bundled `trace-003`, whose
instructions say "Use numbered steps" and whose output has none, qwen3.6 scored
Format Compliance **1.00** with the reasoning "no specific format was requested"
— while penalising the same trace on Adherence for exactly that omission. The
heuristic scored it 0.00, correctly. A local mid-size judge is inconsistent
across metrics within a single response.

This is the argument for keeping both columns. Where the two scorers agree, the
result is credible. Where they diverge, you have found something worth reading
by hand.

**Judge self-preference is unmeasured.** If one model judges traces produced by
several platforms including its own family, a gap favouring its own family
cannot be distinguished from bias without a second, independent judge.

**Sample sizes are small.** A handful of traces scored once each has no
statistical power. These numbers describe the specific traces in the file, not
the platforms in general.

**Local judging is slow.** Roughly 40 seconds per trace for a 23 GB local model
on consumer hardware — about four minutes for five traces. Use
`--judge heuristic` while iterating on report output.

## Automation

| What | Cost | Trigger |
|---|---|---|
| `/eval` slash command | model usage | you type it |
| `/eval-check` slash command | free | you type it |
| CI (`.github/workflows/ci.yml`) | free | every push and PR |

CI makes no model calls. The LLM judge is covered by unit tests on request shape
and response parsing rather than by calling a real model, so the build stays
free, fast and deterministic.

## Development

```bash
cargo test           # 128 tests: unit + end-to-end CLI
cargo clippy --all-targets
cargo fmt --check
```

Integration tests in `tests/cli.rs` run the built binary against temporary
fixture files using the heuristic judge only, so the suite stays offline and
deterministic. The LLM path is covered by unit tests on request shape and
response parsing rather than by calling a real model in CI.

### Layout

```
src/
  cli.rs          clap definition
  domain/         Trace, Metric, Score, MetricScore, EvalReport
  input/          trace loading, sentinel detection
  scoring/        tokenizer, overlap, format rules, heuristics
  judge/          Judge trait, prompt, response parsing, HTTP client, cache
  report/         table and JSON renderers
```

Two design notes worth knowing if you extend this:

- `Score` is a newtype that can only be built through validation, so anything
  holding one is guaranteed to be in `0.0..=1.0`. Parse, don't validate.
- `Judge` returns `impl Future + Send` rather than being an `async fn` trait, so
  the returned futures stay `tokio::spawn`-able if concurrent judging is added.
  Neither form is object-safe, which is why `AnyJudge` exists for runtime
  dispatch.
- `CachedJudge<J>` is a decorator, not a flag inside each judge, so caching
  composes with judges that do not exist yet.
- Cache keys use FNV-1a rather than `DefaultHasher`, whose output is explicitly
  not stable between Rust releases and would silently orphan every entry on a
  toolchain upgrade.
