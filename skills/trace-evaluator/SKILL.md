---
name: trace-evaluator
description: Evaluate a captured agent trace against five instruction-following metrics (adherence, task relevancy, precision, recall, format compliance) and return scored JSON. Use when asked to evaluate, score, grade, or judge an agent output, a model response, or a trace against its instructions.
---

<!-- skill: trace-evaluator v1 -->

# Trace Evaluator

Score one agent trace on five instruction-following metrics.

## Sentinel marker — emit this first

**Begin every response with this line, exactly, before any other text:**

```
<!-- skill: trace-evaluator v1 -->
```

This marker is how the harness records that the skill fired. Without it, the run
is recorded as `skill_triggered: false` even if you followed everything else.
Emit it whether you were asked for this skill by name or inferred it from the
task.

## Input

You will be given three fields:

- **INSTRUCTIONS** — how the agent was told to answer.
- **TASK** — what the agent was asked to do.
- **OUTPUT** — what the agent actually produced.

## The five metrics

Score each from 0.0 to 1.0.

| Metric | Question |
|---|---|
| `InstructionAdherence` | Did the output do what the instructions told it to do? |
| `TaskRelevancy` | Did the output address the task that was asked? |
| `InstructionPrecision` | Was the output free of content the instructions did not call for? |
| `InstructionRecall` | Did the output cover everything the instructions called for? |
| `FormatCompliance` | Did the output obey any stated formatting requirement? |

## Scoring rules

1. **Judge only what is present.** Do not reward length, confidence, politeness
   or apparent effort. A short correct answer beats a long hedged one.
2. **Score each metric independently.** A trace can be perfectly on-task and
   completely non-compliant with its format. Do not let one metric drag another.
3. **`FormatCompliance` is 1.0 when no format was requested.** If the
   instructions state no formatting requirement, there is nothing to violate.
   Do not score 0.0 for the absence of a requirement.
4. **Separate *what to say* from *how to say it*.** "Reply as JSON" is a format
   requirement, scored under `FormatCompliance`. It is not a content item that
   `InstructionRecall` should also penalise.
5. **Be willing to use the full range.** If an output ignores the task entirely,
   score 0.0. If it does everything asked, score 1.0.

## Output format

Reply with **only** the sentinel line followed by a JSON object. No preamble,
no explanation outside the JSON, no markdown fences.

```
<!-- skill: trace-evaluator v1 -->
{"scores":[
  {"metric":"InstructionAdherence","score":0.0,"reasoning":""},
  {"metric":"TaskRelevancy","score":0.0,"reasoning":""},
  {"metric":"InstructionPrecision","score":0.0,"reasoning":""},
  {"metric":"InstructionRecall","score":0.0,"reasoning":""},
  {"metric":"FormatCompliance","score":0.0,"reasoning":""}
]}
```

Use exactly these five metric names. Keep each `reasoning` under 25 words and
make it specific — name the thing that was missing or present, not "good" or
"bad".

## Worked example

**INSTRUCTIONS:** Reply as JSON.
**TASK:** Give the capital of France.
**OUTPUT:** Sure! The capital of France is Paris.

```
<!-- skill: trace-evaluator v1 -->
{"scores":[
  {"metric":"InstructionAdherence","score":0.0,"reasoning":"Ignored the explicit instruction to reply in JSON."},
  {"metric":"TaskRelevancy","score":1.0,"reasoning":"Correctly identified Paris as the capital of France."},
  {"metric":"InstructionPrecision","score":0.7,"reasoning":"Mostly on point, though 'Sure!' is unrequested filler."},
  {"metric":"InstructionRecall","score":1.0,"reasoning":"Covered the single thing asked for."},
  {"metric":"FormatCompliance","score":0.0,"reasoning":"Plain prose where JSON was explicitly required."}
]}
```

Note that `TaskRelevancy` stays 1.0 even though the format was wrong. The answer
was correct; only its packaging failed. Scoring both at 0.0 would lose the
distinction the metrics exist to capture.
