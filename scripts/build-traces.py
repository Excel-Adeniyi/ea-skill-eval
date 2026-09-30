#!/usr/bin/env python3
"""Assemble captured platform responses into a trace file.

Reads every file in a raw-capture directory named:

    <platform>-<prompting>-<case-id>.txt

for example:

    claude_code-explicit-case-2-format-content-split.txt

and emits a traces.json the evaluator can score. The response body becomes the
trace `output`; instructions and task are pulled from cases.json by case id.

Usage:
    python3 scripts/build-traces.py                      # -> samples/captured.json
    python3 scripts/build-traces.py --out other.json
"""

import argparse
import json
import pathlib
import sys

# What SKILL.md actually demands of the responding agent. These are the
# instructions the capture is scored against — NOT the instructions inside the
# trace being evaluated. The platform's job was to follow the rubric, so that is
# what the metrics must measure.
SKILL_INSTRUCTIONS = (
    "Begin the response with the sentinel marker. "
    "Score exactly five metrics named InstructionAdherence, TaskRelevancy, "
    "InstructionPrecision, InstructionRecall and FormatCompliance. "
    "Reply with only a JSON object and no preamble, commentary or markdown fences. "
    "Keep each reasoning under 25 words."
)

PLATFORMS = {"claude_code", "codex_cli", "qwen_code"}
PROMPTINGS = {"explicit", "implicit"}

# Model recorded per platform. Edit to match what you actually ran.
MODELS = {
    "claude_code": "claude-opus-5",
    "codex_cli": "gpt-5-codex",
    "qwen_code": "qwen3.6",
}


def parse_name(stem):
    """Split '<platform>-<prompting>-<case-id>' into its three parts.

    Case ids contain hyphens, so the split is anchored on the two known
    prefixes rather than a naive split('-').
    """
    for platform in PLATFORMS:
        if not stem.startswith(platform + "-"):
            continue
        remainder = stem[len(platform) + 1 :]
        for prompting in PROMPTINGS:
            if remainder.startswith(prompting + "-"):
                return platform, prompting, remainder[len(prompting) + 1 :]
    return None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--raw", default="samples/cases/raw")
    parser.add_argument("--cases", default="samples/cases/cases.json")
    parser.add_argument("--out", default="samples/captured.json")
    args = parser.parse_args()

    cases = {case["id"]: case for case in json.loads(pathlib.Path(args.cases).read_text())}
    raw_dir = pathlib.Path(args.raw)

    if not raw_dir.exists():
        sys.exit(f"no raw capture directory at {raw_dir}")

    traces, skipped = [], []

    for path in sorted(raw_dir.glob("*.txt")):
        parsed = parse_name(path.stem)
        if parsed is None:
            skipped.append(f"{path.name}: filename does not match <platform>-<prompting>-<case-id>")
            continue

        platform, prompting, case_id = parsed
        case = cases.get(case_id)
        if case is None:
            skipped.append(f"{path.name}: unknown case id {case_id!r}")
            continue

        output = path.read_text().strip()
        if not output:
            skipped.append(f"{path.name}: file is empty")
            continue

        # The task is "evaluate this trace", with the case quoted inside it.
        inner = (
            f"INSTRUCTIONS:\n{case['instructions']}\n\n"
            f"TASK:\n{case['task']}\n\n"
            f"OUTPUT:\n{case['output']}"
        )

        traces.append({
            "id": f"{platform}-{prompting}-{case_id}",
            "instructions": SKILL_INSTRUCTIONS,
            "task": f"Evaluate this agent trace.\n\n{inner}",
            "output": output,
            "platform": platform,
            "model": MODELS.get(platform),
            "prompting": prompting,
            # Left out on purpose: the loader infers it from the sentinel.
            # Add "skill_triggered": true/false by hand to override.
        })

    if skipped:
        print("Skipped:", file=sys.stderr)
        for note in skipped:
            print(f"  - {note}", file=sys.stderr)

    if not traces:
        sys.exit("no usable captures found")

    pathlib.Path(args.out).write_text(json.dumps(traces, indent=2) + "\n")

    by_platform = {}
    for trace in traces:
        by_platform[trace["platform"]] = by_platform.get(trace["platform"], 0) + 1

    print(f"Wrote {len(traces)} trace(s) to {args.out}")
    for platform, count in sorted(by_platform.items()):
        print(f"  {platform}: {count}")


if __name__ == "__main__":
    main()
