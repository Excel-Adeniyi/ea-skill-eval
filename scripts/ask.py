#!/usr/bin/env python3
"""Ask an agent a question and score its answer immediately.

Where `capture.py` asks the agent to *act as* the evaluator, this asks a plain
question and evaluates the answer that comes back — a live loop rather than a
batch run.

    python3 scripts/ask.py --task "What is the capital of France?"
    python3 scripts/ask.py --instructions "Reply as JSON." --task "Capital of France?"
    python3 scripts/ask.py --platform codex_cli --task "Explain Rust borrowing" --judge llm

The three fields the evaluator needs map like this:
  --instructions  how the answer should be given   (the system-prompt-ish part)
  --task          what is being asked              (the question)
  output          whatever the agent replies       (captured automatically)

That separation is the whole reason this is not fully automatic: a freeform
chat message does not tell you which half is instruction and which is task, and
guessing would make every score meaningless. You supply the split.
"""

import argparse
import json
import pathlib
import subprocess
import sys
import tempfile
import time

COMMANDS = {
    "claude_code": ["claude", "-p"],
    "codex_cli": ["codex", "exec", "-"],
}

MODELS = {"claude_code": "claude-opus-5", "codex_cli": "gpt-5-codex"}


def ask(platform, prompt, timeout):
    try:
        completed = subprocess.run(
            COMMANDS[platform], input=prompt, capture_output=True, text=True, timeout=timeout
        )
    except subprocess.TimeoutExpired:
        sys.exit(f"{platform} timed out after {timeout}s")
    except FileNotFoundError:
        sys.exit(f"{COMMANDS[platform][0]} is not installed")

    if completed.returncode != 0:
        sys.exit(f"{platform} failed: {(completed.stderr or '').strip()[:300]}")

    answer = completed.stdout.strip()
    if not answer:
        sys.exit(f"{platform} returned nothing")

    return answer


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--task", required=True, help="The question to ask.")
    parser.add_argument("--instructions", default="Answer accurately and concisely.",
                        help="How the answer should be given.")
    parser.add_argument("--platform", default="claude_code", choices=sorted(COMMANDS))
    parser.add_argument("--judge", default="heuristic", choices=["heuristic", "llm"])
    parser.add_argument("--timeout", type=int, default=300)
    parser.add_argument("--keep", help="Also write the trace to this path.")
    args = parser.parse_args()

    prompt = f"{args.instructions}\n\n{args.task}"

    print(f"Asking {args.platform} ...", flush=True)
    started = time.time()
    answer = ask(args.platform, prompt, args.timeout)
    elapsed = time.time() - started

    print(f"\n--- answer ({elapsed:.1f}s) ---\n{answer}\n")

    trace = [{
        "id": "live",
        "instructions": args.instructions,
        "task": args.task,
        "output": answer,
        "platform": args.platform,
        "model": MODELS[args.platform],
        "prompting": "explicit",
    }]

    if args.keep:
        pathlib.Path(args.keep).write_text(json.dumps(trace, indent=2) + "\n")
        trace_path = args.keep
    else:
        handle = tempfile.NamedTemporaryFile("w", suffix=".json", delete=False)
        json.dump(trace, handle, indent=2)
        handle.close()
        trace_path = handle.name

    print("--- evaluation ---", flush=True)
    result = subprocess.run(
        ["cargo", "run", "--quiet", "--", trace_path, "--judge", args.judge],
        text=True,
    )
    sys.exit(result.returncode)


if __name__ == "__main__":
    main()
