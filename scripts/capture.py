#!/usr/bin/env python3
"""Drive agent CLIs non-interactively and save their responses as raw captures.

Replaces the copy-paste loop: runs every case against every requested platform,
in both prompting modes, and writes files that `build-traces.py` understands.

    python3 scripts/capture.py --platform claude_code
    python3 scripts/capture.py --platform claude_code --platform codex_cli
    python3 scripts/capture.py --platform claude_code --prompting explicit
    python3 scripts/capture.py --platform claude_code --case case-1-baseline

Each run costs real model usage. Use --dry-run first to see what would execute.
"""

import argparse
import json
import pathlib
import subprocess
import sys
import time

SKILL_PATH = "skills/trace-evaluator/SKILL.md"

# How to invoke each platform non-interactively. The prompt is fed on stdin,
# not as an argument: these prompts are kilobytes long and start with text the
# CLIs would otherwise try to parse as flags.
COMMANDS = {
    "claude_code": ["claude", "-p"],
    "codex_cli": ["codex", "exec", "-"],
}


def strip_frontmatter(text):
    """Drop the YAML frontmatter block from a SKILL.md body.

    The frontmatter is metadata for the platform's skill loader, not part of
    the rubric, and a prompt beginning with `---` is read as a command-line
    flag by every CLI here.
    """
    if not text.startswith("---"):
        return text

    end = text.find("\n---", 3)
    if end == -1:
        return text

    return text[end + 4 :].lstrip()


def build_prompt(case, prompting, skill_text):
    """The text sent to the agent.

    The rubric is inlined rather than relied upon being auto-loaded, because
    platforms disagree on where instruction files live and a capture run that
    silently used no skill at all is worse than useless. The explicit/implicit
    distinction is about whether the *request* names the skill, which is the
    variable under test.
    """
    trace = (
        f"INSTRUCTIONS:\n{case['instructions']}\n\n"
        f"TASK:\n{case['task']}\n\n"
        f"OUTPUT:\n{case['output']}"
    )

    if prompting == "explicit":
        ask = "Use the trace-evaluator skill to evaluate this trace:"
    else:
        ask = "Evaluate this agent trace:"

    return f"{skill_text}\n\n---\n\n{ask}\n\n{trace}"


def run_one(platform, prompt, timeout):
    command = COMMANDS[platform]

    try:
        completed = subprocess.run(
            command, input=prompt, capture_output=True, text=True, timeout=timeout
        )
    except subprocess.TimeoutExpired:
        return None, f"timed out after {timeout}s"
    except FileNotFoundError:
        return None, f"{command[0]} is not installed"

    if completed.returncode != 0:
        detail = (completed.stderr or completed.stdout or "").strip()[:200]
        return None, f"exit {completed.returncode}: {detail}"

    output = completed.stdout.strip()
    if not output:
        return None, "produced no output"

    return output, None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--platform", action="append", choices=sorted(COMMANDS),
                        help="Repeatable. Defaults to every supported platform.")
    parser.add_argument("--prompting", action="append", choices=["explicit", "implicit"],
                        help="Repeatable. Defaults to both.")
    parser.add_argument("--case", action="append", help="Repeatable. Defaults to all cases.")
    parser.add_argument("--cases", default="samples/cases/cases.json")
    parser.add_argument("--raw", default="samples/cases/raw")
    parser.add_argument("--timeout", type=int, default=300)
    parser.add_argument("--dry-run", action="store_true", help="List runs without executing.")
    parser.add_argument("--force", action="store_true", help="Re-run cases already captured.")
    parser.add_argument(
        "--allow-missing-sentinel",
        action="store_true",
        help="Keep going when a response lacks the sentinel. Needed for implicit "
             "runs, where a skill that does not fire is the result being measured.",
    )
    args = parser.parse_args()

    platforms = args.platform or sorted(COMMANDS)
    promptings = args.prompting or ["explicit", "implicit"]
    cases = json.loads(pathlib.Path(args.cases).read_text())

    if args.case:
        wanted = set(args.case)
        cases = [case for case in cases if case["id"] in wanted]
        if not cases:
            sys.exit(f"no cases matched {sorted(wanted)}")

    skill_text = strip_frontmatter(pathlib.Path(SKILL_PATH).read_text())
    raw_dir = pathlib.Path(args.raw)
    raw_dir.mkdir(parents=True, exist_ok=True)

    planned = [
        (platform, prompting, case)
        for platform in platforms
        for prompting in promptings
        for case in cases
    ]

    print(f"{len(planned)} run(s): {len(platforms)} platform(s) x "
          f"{len(promptings)} prompting mode(s) x {len(cases)} case(s)\n")

    if args.dry_run:
        for platform, prompting, case in planned:
            print(f"  would run {platform}-{prompting}-{case['id']}")
        return

    succeeded = failed = skipped = 0

    for index, (platform, prompting, case) in enumerate(planned, start=1):
        name = f"{platform}-{prompting}-{case['id']}"
        path = raw_dir / f"{name}.txt"

        if path.exists() and not args.force:
            print(f"[{index}/{len(planned)}] {name}: already captured, skipping")
            skipped += 1
            continue

        print(f"[{index}/{len(planned)}] {name} ... ", end="", flush=True)
        started = time.time()
        output, error = run_one(platform, build_prompt(case, prompting, skill_text), args.timeout)
        elapsed = time.time() - started

        if error:
            print(f"FAILED ({error})")
            failed += 1
            continue

        has_sentinel = "trace-evaluator" in output.lower()
        path.write_text(output + "\n")
        print(f"ok {elapsed:.1f}s, {len(output)} chars, "
              f"{'sentinel' if has_sentinel else 'NO SENTINEL'}")
        succeeded += 1

        if not has_sentinel and not args.allow_missing_sentinel:
            print(
                f"\nSTOPPED: {name} came back without the sentinel marker.\n"
                f"  Saved to {path} so you can inspect it.\n\n"
                f"  The skill did not load, so this run measures the model's "
                f"general behaviour\n  rather than the rubric. Continuing would "
                f"spend usage on data that\n  answers a different question.\n\n"
                f"  Check: is {SKILL_PATH} installed for {platform}?\n"
                f"  If a non-firing skill is what you are measuring (implicit "
                f"prompting),\n  re-run with --allow-missing-sentinel.",
                file=sys.stderr,
            )
            sys.exit(2)

    print(f"\n{succeeded} captured, {failed} failed, {skipped} skipped -> {raw_dir}")

    if succeeded:
        print("\nNext:\n  python3 scripts/build-traces.py"
              "\n  cargo run -- samples/captured.json --judge llm")

    if failed:
        sys.exit(1)


if __name__ == "__main__":
    main()
