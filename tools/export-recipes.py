#!/usr/bin/env python3
"""Export complete public-API recipes from the committed learning example."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parent.parent
IDS = {"composer", "workbar", "ocean", "whale", "animated-whale"}


def committed_recipes():
    revision = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
    ).strip()
    source = subprocess.check_output(
        ["git", "show", f"{revision}:examples/recipes.rs"], cwd=ROOT, encoding="utf-8"
    )
    pattern = re.compile(
        r"^// recipe:([a-z-]+):start\n(.*?)^// recipe:\1:end$",
        re.MULTILINE | re.DOTALL,
    )
    recipes = {}
    for match in pattern.finditer(source):
        name, code = match.groups()
        if name in recipes or name not in IDS:
            raise ValueError(f"Invalid or duplicated recipe: {name}")
        if not code.startswith("pub fn draw_"):
            raise ValueError(f"Recipe is not a public drawing function: {name}")
        recipes[name] = {
            "code": code.rstrip() + "\n",
            "line": source.count("\n", 0, match.start(2)) + 1,
        }
    if recipes.keys() != IDS:
        raise ValueError(f"Missing recipes: {sorted(IDS - recipes.keys())}")
    return {
        "sourceRevision": revision,
        "sourceSha256": hashlib.sha256(source.encode()).hexdigest(),
        "recipes": recipes,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    output = json.dumps(committed_recipes(), ensure_ascii=False, indent=2) + "\n"
    if args.check:
        if not args.output.exists() or args.output.read_text(encoding="utf-8") != output:
            raise SystemExit("Recipe export is out of date")
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(output, encoding="utf-8")


if __name__ == "__main__":
    main()
