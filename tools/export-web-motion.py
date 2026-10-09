#!/usr/bin/env python3
"""Export verified native animation frames for the Codewhale website.

    python3 -B tools/export-web-motion.py ../CodeWhale/web/public/ratatui/motion
    python3 -B tools/export-web-motion.py ../CodeWhale/web/public/ratatui/motion --check

The six native examples must already have exported their frames to target/.
check-animation.py verifies every frame against the committed README GIF record
before this exporter writes anything. The browser receives the unchanged SVGs,
the native cell dimensions and the original frame interval. No raster tools,
network requests, timestamps or JavaScript animation approximations are involved.
"""

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import subprocess
import sys
import xml.etree.ElementTree as ET


ROOT = Path(__file__).resolve().parents[1]
REPOSITORY = "https://github.com/Hmbown/codewhale-ratatui"
# Keep comfortably within the host's 25 MB limit for an individual static asset.
MAX_ASSET_BYTES = 25_000_000
SVG_NAMESPACE = "{http://www.w3.org/2000/svg}"
SVG_TAGS = {"svg", "title", "g", "rect", "text"}
TRACKS = (
    {"id": "studio", "title": "Native work session", "profile": "dark-truecolor",
     "frames": "target/showcase-frames", "gif": "assets/readme/showcase.gif", "example": "examples/showcase.rs"},
    {"id": "studio-light", "title": "Native work session · light", "profile": "light-truecolor",
     "frames": "target/showcase-light-frames", "gif": "assets/readme/showcase-light.gif", "example": "examples/showcase.rs"},
    {"id": "whale", "title": "Seventeen whale performances", "profile": "dark-truecolor",
     "frames": "target/whale-action-frames", "gif": "assets/readme/whale-performance.gif", "example": "examples/showcase.rs"},
    {"id": "habitat", "title": "Marine habitat", "profile": "dark-truecolor",
     "frames": "target/habitat-frames", "gif": "assets/readme/habitat-motion.gif", "example": "examples/habitat.rs"},
    {"id": "motion", "title": "Spinners and verification", "profile": "dark-truecolor",
     "frames": "target/motion-frames", "gif": "assets/readme/motion-demo.gif", "example": "examples/motion.rs"},
    {"id": "motion-light", "title": "Spinners and verification · light", "profile": "light-truecolor",
     "frames": "target/motion-light-frames", "gif": "assets/readme/motion-demo-light.gif", "example": "examples/motion.rs"},
)


def animation_checker():
    spec = importlib.util.spec_from_file_location("check_animation", ROOT / "tools/check-animation.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def canonical_json(value):
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")) + "\n").encode("utf-8")


def source_revision(root):
    """A docs/exporter-only commit must not change the native source links."""
    paths = ["src", "assets/whale-v2.scenes", "assets/whale-motion", "assets/tui-palettes.json",
             "assets/tui-source.json", "vendor/codewhale-design",
             *sorted({track["example"] for track in TRACKS}),
             *sorted({track["gif"] for track in TRACKS}),
             *sorted({track["gif"] + ".json" for track in TRACKS})]
    dirty = subprocess.check_output(
        ["git", "status", "--porcelain", "--", *paths], cwd=root, text=True,
    ).strip()
    if dirty:
        raise ValueError("commit the native animation sources and GIF records before exporting immutable source links")
    revision = subprocess.check_output(
        ["git", "log", "-1", "--format=%H", "--", *paths], cwd=root, text=True,
    ).strip()
    if not re.fullmatch(r"[0-9a-f]{40}", revision):
        raise ValueError("expected an immutable native source commit")
    return revision


def validate_svg(svg, width, height):
    # Parsing is only validation. The exported SVG string remains byte-for-byte
    # identical after JSON decoding, including its native colors and text.
    if "<!DOCTYPE" in svg.upper() or "<!ENTITY" in svg.upper():
        raise ValueError("unsupported SVG declarations")
    element = ET.fromstring(svg)
    if element.tag != SVG_NAMESPACE + "svg" or element.get("viewBox") != f"0 0 {width * 10} {height * 20}":
        raise ValueError("SVG geometry differs from the native frame manifest")
    for node in element.iter():
        if node.tag not in {SVG_NAMESPACE + tag for tag in SVG_TAGS}:
            raise ValueError("unsupported active or external SVG element")
        for key, value in node.attrib.items():
            attribute = key.rsplit("}", 1)[-1].lower()
            if attribute.startswith("on") or attribute in {"href", "style"} or "url(" in value.lower():
                raise ValueError("unsupported active or external SVG attribute")


def export_track(track, root, revision, checker):
    source, gif = root / track["frames"], root / track["gif"]
    # Reuses the existing 2–600 frame, path, profile, dimensions, time and hash
    # validation instead of introducing another animation manifest authority.
    checker.check(source, gif)
    manifest = json.loads((source / "manifest.json").read_text(encoding="utf-8"))
    first = manifest[0]
    if first["profile"] != track["profile"]:
        raise ValueError(f"{track['id']}: terminal profile differs from the named track")
    frames = []
    for frame in manifest:
        svg = (source / frame["file"]).read_text(encoding="utf-8")
        validate_svg(svg, first["width"], first["height"])
        frames.append(svg)
    frame_ms = manifest[1]["elapsed_ms"] - first["elapsed_ms"]
    value = {
        "id": track["id"], "title": track["title"], "profile": first["profile"],
        "width": first["width"], "height": first["height"], "frameMs": frame_ms,
        "frames": frames, "source": f"{REPOSITORY}/blob/{revision}/{track['example']}",
    }
    payload = canonical_json(value)
    if len(payload) > MAX_ASSET_BYTES:
        raise ValueError(f"{track['id']}: motion asset exceeds {MAX_ASSET_BYTES:,} bytes; split the native track")
    record = json.loads(Path(str(gif) + ".json").read_text(encoding="utf-8"))
    metadata = {
        "id": track["id"], "file": track["id"] + ".json", "profile": first["profile"],
        "frames": len(frames), "width": first["width"], "height": first["height"],
        "frameMs": frame_ms, "startMs": first["elapsed_ms"], "bytes": len(payload),
        "sha256": hashlib.sha256(payload).hexdigest(), "sourceSha256": record["source_sha256"],
        "gifSha256": record["gif_sha256"], "source": value["source"],
    }
    return payload, metadata


def generate(root=ROOT):
    revision, checker = source_revision(root), animation_checker()
    output, records = {}, []
    # Validate every track before touching the website directory.
    for track in TRACKS:
        payload, metadata = export_track(track, root, revision, checker)
        output[metadata["file"]] = payload
        records.append(metadata)
    output["index.json"] = canonical_json({
        "schemaVersion": 1, "repository": REPOSITORY, "sourceRevision": revision, "tracks": records,
    })
    return output, records


def write(output, destination, check=False):
    mismatches = [name for name, payload in output.items()
                  if not (destination / name).is_file() or (destination / name).read_bytes() != payload]
    if check:
        if mismatches:
            raise ValueError("website motion is stale: " + ", ".join(mismatches))
        return
    destination.mkdir(parents=True, exist_ok=True)
    # Only these seven named artifacts are owned. Neighboring data is preserved.
    for name, payload in output.items():
        if name in mismatches:
            (destination / name).write_bytes(payload)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("destination", type=Path)
    parser.add_argument("--check", action="store_true", help="fail if the website data differs; write nothing")
    args = parser.parse_args()
    output, records = generate()
    write(output, args.destination, args.check)
    for record in records:
        print(f"{record['file']}: {record['frames']} frames, {record['width']}×{record['height']} cells, "
              f"{record['frameMs']} ms, {record['bytes']:,} bytes")
    print(f"{'Verified' if args.check else 'Exported'} {len(records)} native tracks / "
          f"{sum(record['frames'] for record in records)} frames")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, TypeError, ET.ParseError, subprocess.CalledProcessError) as error:
        sys.exit(str(error))
