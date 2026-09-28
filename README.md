# ea-skill-eval

A command-line harness for evaluating agent traces on five instruction-following
metrics, with two independent scorers: fast offline heuristics, and any
OpenAI-compatible model acting as a judge.

Built to compare how the same portable skill behaves across different agent
platforms — Claude Code, Codex CLI, Qwen Code — by scoring the traces each one
produces.

## Quick start

```bash
# Offline heuristics against the bundled samples. Instant.
cargo run

# Judge with a local model through Ollama.
cargo run -- --judge llm --model qwen3.6:latest

# Machine-readable output for CI.
cargo run -- --format json

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
```

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

## Development

```bash
cargo test           # 107 tests: unit + end-to-end CLI
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
  input/          trace loading
  scoring/        tokenizer, overlap, format rules, heuristics
  judge/          Judge trait, prompt, response parsing, HTTP client
  report/         table and JSON renderers
```

Two design notes worth knowing if you extend this:

- `Score` is a newtype that can only be built through validation, so anything
  holding one is guaranteed to be in `0.0..=1.0`. Parse, don't validate.
- `Judge` returns `impl Future + Send` rather than being an `async fn` trait, so
  the returned futures stay `tokio::spawn`-able if concurrent judging is added.
  Neither form is object-safe, which is why `AnyJudge` exists for runtime
  dispatch.
