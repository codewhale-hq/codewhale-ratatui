#!/usr/bin/env python3
"""Verify the README animation against the current actual-buffer export.

No raster dependencies are needed for CI: a changed SVG frame invalidates
the recorded source hash, and an edited animation invalidates its file hash.
"""
import hashlib
import json
from pathlib import Path
import sys
import argparse

PROFILES = {"dark-truecolor", "dark-graphite", "light-truecolor", "dark-256", "light-256", "ansi-16", "unknown-ground", "no-color", "ascii"}


def check(source, gif):
    manifest = (source / "manifest.json").read_bytes()
    frames = json.loads(manifest)
    if not isinstance(frames, list) or not 2 <= len(frames) <= 600:
        raise ValueError("expected 2–600 exported frames")
    first = frames[0]
    if not isinstance(first, dict):
        raise ValueError("expected a frame object")
    def integer(key, minimum, maximum):
        value = first.get(key)
        return type(value) is int and minimum <= value <= maximum
    if not integer("width", 1, 160) or not integer("height", 1, 80) or not integer("elapsed_ms", 0, 2**53 - 1) or first.get("profile") not in PROFILES:
        raise ValueError("invalid frame dimensions, profile or time")
    if not isinstance(frames[1], dict) or type(frames[1].get("elapsed_ms")) is not int:
        raise ValueError("invalid second frame time")
    interval = frames[1]["elapsed_ms"] - first["elapsed_ms"]
    if not 20 <= interval <= 1000 or interval % 10:
        raise ValueError("frame interval must be 20–1000 ms in GIF centiseconds")
    digest = hashlib.sha256(manifest)
    for index, frame in enumerate(frames):
        expected = {"file": f"frame-{index:03}.svg", "elapsed_ms": first["elapsed_ms"] + index * interval,
                    "profile": first["profile"], "width": first["width"], "height": first["height"]}
        if frame != expected or any(type(frame.get(key)) is not int for key in ("elapsed_ms", "width", "height")) or frame["elapsed_ms"] > 2**53 - 1:
            raise ValueError("unexpected frame manifest")
        digest.update(frame["file"].encode())
        digest.update(b"\0")
        digest.update((source / frame["file"]).read_bytes())
    expected = {"source_sha256": digest.hexdigest(), "gif_sha256": hashlib.sha256(gif.read_bytes()).hexdigest(),
                "frames": len(frames), "width": first["width"] * 10, "height": first["height"] * 20, "frame_ms": interval}
    if json.loads(Path(str(gif) + ".json").read_text()) != expected:
        raise ValueError("animation is stale; regenerate actual frames and render-animation.cjs")
    print(f"{gif.name} matches all {len(frames)} current terminal-buffer frames")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("gif", type=Path)
    args = parser.parse_args()
    try:
        check(args.source, args.gif)
    except (OSError, ValueError, KeyError, TypeError) as error:
        sys.exit(str(error))
