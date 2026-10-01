#!/usr/bin/env python3
"""Stop-hook scorer: evaluate the response Claude just gave.

Reads the hook payload on stdin, pulls the last user prompt and the last
assistant reply out of the transcript, scores them with the heuristic scorer,
and returns a one-line systemMessage.

Deliberately constrained:

* Heuristic only. The LLM judge takes ~75s per trace; a Stop hook that slow
  would make every turn unusable.
* Never fails the turn. Any problem exits 0 with no output — a scorer is not
  worth blocking work over.
* Silent when it has nothing useful to say (no transcript, trivial reply).

The honest caveat: the evaluator needs instructions, task and output as three
separate things, and a chat turn only supplies two. The "instructions" here are
a fixed standing policy, not something you actually said, so treat the score as
a drift signal across turns rather than a verdict on any one answer. Override it
with SKILL_EVAL_INSTRUCTIONS.
"""

import json
import os
import pathlib
import subprocess
import sys
import tempfile

DEFAULT_INSTRUCTIONS = "Answer the question directly and accurately, without padding or unrequested content."

# Below this, a reply is an acknowledgement rather than an answer worth scoring.
MIN_OUTPUT_CHARS = 120


def quiet_exit():
    """Say nothing, block nothing."""
    sys.exit(0)


def text_of(content):
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        return "\n".join(
            block.get("text", "")
            for block in content
            if isinstance(block, dict) and block.get("type") == "text"
        )
    return ""


def last_exchange(transcript_path):
    """The final user prompt and assistant reply, or (None, None)."""
    user_text = assistant_text = None

    try:
        lines = pathlib.Path(transcript_path).read_text().splitlines()
    except OSError:
        return None, None

    for line in reversed(lines):
        try:
            entry = json.loads(line)
        except json.JSONDecodeError:
            continue

        kind = entry.get("type")
        message = entry.get("message")
        if not isinstance(message, dict):
            continue

        body = text_of(message.get("content")).strip()
        if not body:
            continue

        if kind == "assistant" and assistant_text is None:
            assistant_text = body
        elif kind == "user" and user_text is None:
            # Tool results come back as user-role entries; they are not prompts.
            if body.startswith("<") and "tool_use_id" in body:
                continue
            user_text = body

        if user_text and assistant_text:
            break

    return user_text, assistant_text


JUDGE_PROMPT = """You are an evaluation judge. Score the ANSWER against the QUESTION on five metrics, each 0.0 to 1.0:
- InstructionAdherence: did the answer do what was asked?
- TaskRelevancy: did it address the question?
- InstructionPrecision: was it free of padding and unrequested content?
- InstructionRecall: did it cover everything asked?
- FormatCompliance: did it obey any stated format? Score 1.0 if no format was requested.

Reply with ONLY this JSON and no other text:
{{"scores":[{{"metric":"InstructionAdherence","score":0.0,"reasoning":""}},{{"metric":"TaskRelevancy","score":0.0,"reasoning":""}},{{"metric":"InstructionPrecision","score":0.0,"reasoning":""}},{{"metric":"InstructionRecall","score":0.0,"reasoning":""}},{{"metric":"FormatCompliance","score":0.0,"reasoning":""}}]}}
Keep each reasoning under 15 words.

QUESTION:
{question}

ANSWER:
{answer}
"""


def judge_with_codex(task, output, timeout):
    """Score with Codex, an independent model.

    Codex rather than Claude on purpose: a model scoring its own answers has a
    self-preference bias, and the whole point of a judge is independence.
    Returns None on any failure so the caller falls back to the heuristic.
    """
    prompt = JUDGE_PROMPT.format(question=task[:3000], answer=output[:6000])

    try:
        completed = subprocess.run(
            ["codex", "exec", "-"],
            input=prompt, capture_output=True, text=True, timeout=timeout,
        )
    except (subprocess.TimeoutExpired, FileNotFoundError, OSError):
        return None

    if completed.returncode != 0:
        return None

    raw = completed.stdout
    start, end = raw.find("{"), raw.rfind("}")
    if start == -1 or end <= start:
        return None

    try:
        parsed = json.loads(raw[start:end + 1])
        entries = parsed["scores"]
    except (json.JSONDecodeError, KeyError, TypeError):
        return None

    scores = []
    for entry in entries:
        try:
            value = float(entry["score"])
        except (KeyError, TypeError, ValueError):
            continue
        if 0.0 <= value <= 1.0:
            scores.append({"metric": str(entry.get("metric", "?")), "score": value})

    return scores or None


def main():
    try:
        payload = json.load(sys.stdin)
    except (json.JSONDecodeError, ValueError):
        quiet_exit()

    transcript_path = payload.get("transcript_path")
    if not transcript_path:
        quiet_exit()

    task, output = last_exchange(transcript_path)
    if not task or not output or len(output) < MIN_OUTPUT_CHARS:
        quiet_exit()

    judge_timeout = int(os.environ.get("SKILL_EVAL_JUDGE_TIMEOUT", "60"))
    scores = judge_with_codex(task, output, judge_timeout)
    source = "codex"

    if scores is None:
        # Fall back to the offline scorer so a turn is never left unscored.
        source = "heuristic"
        project = pathlib.Path(__file__).resolve().parent.parent
        binary = project / "target" / "debug" / "ea-skill-eval"
        if not binary.exists():
            quiet_exit()

        trace = [{
            "id": "turn",
            "instructions": os.environ.get("SKILL_EVAL_INSTRUCTIONS", DEFAULT_INSTRUCTIONS),
            "task": task[:4000],
            "output": output[:8000],
            "platform": "claude_code",
            "prompting": "implicit",
        }]

        handle = tempfile.NamedTemporaryFile("w", suffix=".json", delete=False)
        json.dump(trace, handle)
        handle.close()

        try:
            completed = subprocess.run(
                [str(binary), handle.name, "--format", "json"],
                capture_output=True, text=True, timeout=20,
            )
            scores = json.loads(completed.stdout)["evaluations"][0]["scores"]
        except Exception:
            quiet_exit()
        finally:
            os.unlink(handle.name)

    if not scores:
        quiet_exit()

    short = {
        "InstructionAdherence": "adher",
        "TaskRelevancy": "relev",
        "InstructionPrecision": "prec",
        "InstructionRecall": "recall",
        "FormatCompliance": "format",
    }

    # Single line on purpose: systemMessage renders one line, so a bordered
    # table is silently dropped rather than shown.
    parts = [f"{short.get(s['metric'], s['metric'])} {s['score']:.2f}" for s in scores]
    mean = sum(s["score"] for s in scores) / len(scores)
    message = f"eval ({source})  mean {mean:.2f}  │  " + "   ".join(parts)

    # The full table goes to a file, for anyone who wants to watch it.
    try:
        rows = "\n".join(f"  {s['metric']:<22} {s['score']:>5.2f}" for s in scores)
        pathlib.Path("/tmp/skill-eval-last.txt").write_text(
            f"{task[:80]}\n\n{rows}\n  {'Mean':<22} {mean:>5.2f}\n"
        )
    except OSError:
        pass

    print(json.dumps({"systemMessage": message}))


if __name__ == "__main__":
    main()
