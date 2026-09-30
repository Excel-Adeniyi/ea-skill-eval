# Agent instructions

This repository carries one portable skill, defined once in
[`skills/trace-evaluator/SKILL.md`](skills/trace-evaluator/SKILL.md).

## Loading the skill

**Read `skills/trace-evaluator/SKILL.md` now and follow it** whenever a request
asks you to evaluate, score, grade or judge an agent output, a model response,
or a trace against its instructions.

That file is the single source of truth. This file exists only to point at it,
so that the rubric does not drift between platforms.

## Why the indirection

Agent surfaces disagree on where instructions live. Claude Code loads
`SKILL.md` with YAML frontmatter from a skills directory; Codex CLI and several
others read `AGENTS.md` from the repository root. Rather than maintain two
copies of the rubric — which would silently diverge and invalidate any
comparison between platforms — the rubric lives in one file and each platform's
entry point points to it.

If you are comparing platforms using this repository, a difference in scores
should come from the model, not from the two of them having read different
instructions.

## The sentinel marker

`SKILL.md` requires every response to begin with:

```
<!-- skill: trace-evaluator v1 -->
```

This is not decoration. It is the only reliable way to record whether the skill
actually fired, as opposed to the model answering from general knowledge. It
matters most when the skill was *not* requested by name: the experiment this
repository supports is whether the skill triggers under implicit prompting, and
that question has no answer without an observable marker.

Emit it whether you were asked for the skill explicitly or inferred it.
