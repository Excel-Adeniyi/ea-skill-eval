# Capture prompts

Paste one block per run. Save each response to
`samples/cases/raw/<platform>-<prompting>-<case-id>.txt`.

Platforms: `claude_code`, `codex_cli`, `qwen_code`.


## case-1-baseline

_Targets: Control. Everything correct; both platforms should score this well._

### Explicit (names the skill)

```
Use the trace-evaluator skill to evaluate this trace:

INSTRUCTIONS:
Answer in one sentence.

TASK:
Explain what a Rust struct is.

OUTPUT:
A struct groups related values under a single named type.
```

### Implicit (does not name the skill)

```
Evaluate this agent trace:

INSTRUCTIONS:
Answer in one sentence.

TASK:
Explain what a Rust struct is.

OUTPUT:
A struct groups related values under a single named type.
```


## case-2-format-content-split

_Targets: SKILL.md rule 2: score metrics independently. TaskRelevancy should stay high while FormatCompliance is 0._

### Explicit (names the skill)

```
Use the trace-evaluator skill to evaluate this trace:

INSTRUCTIONS:
Reply as JSON.

TASK:
Give the capital of France.

OUTPUT:
Sure! The capital of France is Paris.
```

### Implicit (does not name the skill)

```
Evaluate this agent trace:

INSTRUCTIONS:
Reply as JSON.

TASK:
Give the capital of France.

OUTPUT:
Sure! The capital of France is Paris.
```


## case-3-no-format-requested

_Targets: SKILL.md rule 3: FormatCompliance is 1.0 when no format was requested. qwen3.6 got this wrong locally._

### Explicit (names the skill)

```
Use the trace-evaluator skill to evaluate this trace:

INSTRUCTIONS:
Be accurate.

TASK:
Name the largest planet in the solar system.

OUTPUT:
Jupiter is the largest planet in the solar system.
```

### Implicit (does not name the skill)

```
Evaluate this agent trace:

INSTRUCTIONS:
Be accurate.

TASK:
Name the largest planet in the solar system.

OUTPUT:
Jupiter is the largest planet in the solar system.
```


## case-4-meta-instruction-trap

_Targets: SKILL.md rule 4: 'Reply as JSON' is a format requirement, not a content item recall should also penalise._

### Explicit (names the skill)

```
Use the trace-evaluator skill to evaluate this trace:

INSTRUCTIONS:
Reply as JSON with a single key called answer.

TASK:
What is 2 + 2?

OUTPUT:
{"answer": 4}
```

### Implicit (does not name the skill)

```
Evaluate this agent trace:

INSTRUCTIONS:
Reply as JSON with a single key called answer.

TASK:
What is 2 + 2?

OUTPUT:
{"answer": 4}
```


## case-5-total-miss

_Targets: SKILL.md rule 5: use the full range. An output ignoring the task entirely should score 0.0, not 0.3._

### Explicit (names the skill)

```
Use the trace-evaluator skill to evaluate this trace:

INSTRUCTIONS:
Mention lifetimes.

TASK:
Explain why Rust needs lifetimes.

OUTPUT:
The weather today is pleasant and I enjoyed a long walk.
```

### Implicit (does not name the skill)

```
Evaluate this agent trace:

INSTRUCTIONS:
Mention lifetimes.

TASK:
Explain why Rust needs lifetimes.

OUTPUT:
The weather today is pleasant and I enjoyed a long walk.
```
