#!/usr/bin/env python3
"""Compose README boards from real gallery SVG buffers, using only the stdlib.

    cargo run --example gallery -- --svg target/readme-buffers
    python3 tools/render-gallery.py target/readme-buffers assets/readme
    python3 tools/render-gallery.py target/readme-buffers assets/readme --check

Every input entry appears once in a board for each requested profile. The
default profiles are dark-truecolor (Ocean) and light-truecolor; --profiles accepts
any exported profiles. Whale states have their own boards, and the complete
action sheet stands alone. SVGs embed the buffer drawing directly and need no
JavaScript, foreignObject, remote image or font resource to display on GitHub.
"""

import argparse
import copy
import json
import math
import os
from pathlib import Path
import re
import sys
import textwrap
import xml.etree.ElementTree as ET

SVG = "http://www.w3.org/2000/svg"
ET.register_namespace("", SVG)
ROOT = Path(__file__).resolve().parents[1]
TOKENS = json.loads((ROOT / "vendor/codewhale-design/tokens.json").read_text(encoding="utf-8"))
NATIVE_PALETTES = json.loads((ROOT / "assets/tui-palettes.json").read_text(encoding="utf-8"))
WIDTH = 1040
MARGIN = TOKENS["spacing"]["page"]
GAP = TOKENS["spacing"]["large"]
TOP = 96
MAX_HEIGHT = 2200
MONO = "'DejaVu Sans Mono','Cascadia Mono','SFMono-Regular',Consolas,'Liberation Mono',monospace"
SANS = ",".join(f"'{family}'" for family in [TOKENS["typography"]["family"], *TOKENS["typography"]["fallbacks"]]) + ",-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif"
GROUPS = {
    "studio": ("The live component gallery", "The native conversation layout, composer and workbar, plus interactive component studies"),
    "pet": ("The animated whale pet", "The native cove, reading, attention and pod, plus the full pet surface"),
    "native-views": ("Codewhale terminal views", "Sessions, settings, pickers and work panels built from reusable native parts"),
    "native-chrome": ("The native composer and footer", "Composer geometry, permission and mode, workflow rows and model/context metrics"),
    "workbar": ("The native workbar", "Tasks, Fleet, Jobs, Files, Notes, Context, Git and Cost; bottom, top and side placement"),
    "tui-palettes": ("Every Codewhale TUI theme", "Sixteen source presets with their actual backgrounds, status, permission and mode inks"),
    "water": ("Codewhale water and ombres", "The current TUI ocean, plus optional Lagoon, Dusk, Coral and Graphite treatments"),
    "transcript": ("Conversation and queued input", "Rich prose, code, attached context and the next instruction"),
    "components": ("Conversation and agents", "Inline messages and optional tool, agent and fleet cards"),
    "foundation": ("The Codewhale language", "Depth, rules, marks, hints and terminal chrome"),
    "input": ("Input and selection", "Editable fields, forms, lists and focused choices"),
    "chrome": ("Navigation and controls", "Headings, tabs, toggles and keyboard maps"),
    "display": ("Work and results", "Diffs, trees, progress, approvals and run results"),
    "motion": ("Motion and feedback", "Spinners, notifications and calm transitions"),
    "habitat": ("Life in the water", "Fish, jellyfish and bubbles, drawn in terminal cells"),
    "whales": ("A whale with a job", "Session state, attention, completion and the pod"),
    "whale-actions": ("Every whale action", "The complete v2 state vocabulary, in terminal cells"),
    "scenes": ("Optional workspace compositions", "Desktop-inspired conversation, review and fleet layouts you can compose from the library"),
}
PROFILES = ["dark-truecolor", "dark-graphite", "light-truecolor", "dark-256", "light-256",
            "ansi-16", "unknown-ground", "no-color", "ascii"]
PROFILE_LABELS = {
    "dark-truecolor": "Truecolor / Ocean", "dark-graphite": "Truecolor / Graphite",
    "light-truecolor": "Truecolor / Paper", "dark-256": "256 colors / Dark",
    "light-256": "256 colors / Light", "ansi-16": "16 colors",
    "unknown-ground": "Unmeasured ground", "no-color": "NO_COLOR", "ascii": "ASCII safe",
}


def element(parent, tag, attrs=None, text=None):
    node = ET.SubElement(parent, f"{{{SVG}}}{tag}", attrs or {})
    if text is not None:
        node.text = text
    return node


def classify(name):
    if name.startswith("pet-"):
        return "pet"
    if name.startswith("tui-theme-"):
        return "tui-palettes"
    if name.startswith(("native-composer", "workflow-", "posture-", "metrics-")):
        return "native-chrome"
    if name.startswith(("native-", "view-")) or name in {"instrument-surface", "session-list"}:
        return "native-views"
    if name.startswith("showcase-"):
        return "studio"
    if name.startswith(("ocean-", "atmosphere-")):
        return "water"
    if name.startswith(("transcript-", "code-block", "pending-")):
        return "transcript"
    if name.startswith(("workspace-scene", "review-scene", "fleet-scene", "habitat-scene")):
        return "scenes"
    if re.search(r"habitat|fish|jelly|bubble", name):
        return "habitat"
    if name == "workbar" or name.startswith("workbar-"):
        return "workbar"
    if name.startswith("artifact"):
        return "display"
    if name == "whale-actions":
        return "whale-actions"
    if name.startswith("whale-"):
        return "whales"
    if name in {"key-hints", "status-marks", "depth", "horizon", "icons"}:
        return "foundation"
    if re.search(r"dialog|sheet", name):
        return "foundation"
    if re.search(r"message|composer|tool-card|fleet|attention", name):
        return "components"
    if re.search(r"input|form|fuzzy|picker|list|empty", name):
        return "input"
    if re.search(r"heading|tabs|toggle|segmented|keymap|setting", name):
        return "chrome"
    if re.search(r"receipt|diff|tree|progress|approval|review|count-bar|artifact", name):
        return "display"
    if re.search(r"motion|spinner|toast|verification", name):
        return "motion"
    return "components"


def palette(profile):
    light = profile.startswith("light")
    native = next(p for p in NATIVE_PALETTES
                  if p["variant"] == ("WhaleLight" if light else "Underwater"))
    colors = TOKENS["colors"]["light" if light else "dark"]
    roles = {"bg": (1, "background"), "surface": (2, "surface"),
             "sidebar": (0, "sidebar"), "edge": (7, "border"),
             "fg": (5, "foreground"), "muted": (6, "muted_foreground"),
             "accent": (9, "primary")}
    # Reset inherits the same measured host ground as the buffer exporter.
    return {name: "#" + ("".join(f"{channel:02x}" for channel in native["roles"][index])
                         if isinstance(native["roles"][index], list)
                         else colors[role])
            for name, (index, role) in roles.items()}


def read_inputs(source, profiles):
    exported = {}
    for profile in profiles:
        suffix = f".{profile}.svg"
        exported[profile] = {
            path.name[:-len(suffix)]: path
            for path in sorted(source.glob(f"*{suffix}"))
        }
        if not exported[profile]:
            raise ValueError(f"no {profile} SVG buffers found in {source}")
    manifest = source / "manifest.json"
    if manifest.exists():
        data = json.loads(manifest.read_text(encoding="utf-8"))
        entries = data.get("entries", data) if isinstance(data, dict) else data
        names = [entry["name"] if isinstance(entry, dict) else entry for entry in entries]
    else:
        names = sorted(exported[profiles[0]])
    if len(names) != len(set(names)):
        raise ValueError("duplicate gallery names in manifest")
    expected = set(names)
    for profile, paths in exported.items():
        missing = expected - paths.keys()
        extra = paths.keys() - expected
        if missing or extra:
            raise ValueError(f"{profile}: missing {sorted(missing)}, extra {sorted(extra)}")
    return names, exported


def load_buffer(path):
    root = ET.fromstring(path.read_text(encoding="utf-8"))
    if root.tag != f"{{{SVG}}}svg":
        raise ValueError(f"{path} is not an SVG buffer")
    # Exported buffers are intentionally self-contained. Refuse an unrelated
    # image or active document accidentally placed in the input directory.
    allowed = {"svg", "title", "desc", "rect", "g", "text", "path"}
    for node in root.iter():
        if node.tag.removeprefix(f"{{{SVG}}}") not in allowed:
            raise ValueError(f"unexpected element {node.tag} in {path}")
        if any(key.lower().startswith("on") or "href" in key.lower() for key in node.attrib):
            raise ValueError(f"active or external SVG content in {path}")
        if any(re.search(r"url\s*\(|javascript:|data:|@import", value, re.IGNORECASE)
               for value in node.attrib.values()):
            raise ValueError(f"external SVG resource in {path}")
    left, top, width, height = (float(v) for v in root.attrib["viewBox"].split())
    if left != 0 or top != 0 or not all(math.isfinite(v) for v in [width, height]) or width <= 0 or height <= 0:
        raise ValueError(f"invalid buffer dimensions in {path}")
    return root, width, height


def specimen_title(name):
    titles = {"status-marks": "State, in a mark and a word", "key-hints": "Keys at the point of use",
              "depth": "One space, several depths", "horizon": "A single horizon",
              "icons": "Control vocabulary", "workspace-scene": "The everyday workspace",
              "review-scene": "Review in context", "fleet-scene": "Parallel work in view",
              "workspace-scene-narrow": "The workspace in a narrow terminal",
              "habitat-scene": "A living marine workspace"}
    if name in titles:
        return titles[name]
    words = name.replace("-", " ")
    for abbreviation in ["ascii", "cjk", "mcp", "api", "ui"]:
        words = re.sub(rf"\b{abbreviation}\b", abbreviation.upper(), words)
    return words[:1].upper() + words[1:]


def label_lines(value, width, size, mono=False):
    # Labels are catalogue names, not product text. A conservative measure
    # keeps every name visible without depending on a host's installed fonts.
    columns = max(1, int(width / (size * (0.625 if mono else 0.65))))
    return textwrap.wrap(value, width=columns, break_long_words=True, break_on_hyphens=True) or [""]


def specimen_labels(name, span, width, height):
    title = label_lines(specimen_title(name), span, TOKENS["typography"]["heading_px"])
    # Dimensions and slugs stay in the manifest and accessible SVG metadata.
    return title, [], len(title) * GAP + TOKENS["spacing"]["section"]


def layout(items, group=None):
    """Fit the specimens rather than shrink their terminal cells to a grid."""
    if not items:
        return []
    if group in {"scenes", "studio", "water", "native-views"}:
        pages = []
        for name, buffer, width, height in items:
            board_width = max(640, math.ceil(width) + 2 * MARGIN)
            span = board_width - 2 * MARGIN
            title, metadata, header = specimen_labels(name, span, width, height)
            pages.append({"width": board_width, "specimens": [{"name": name, "buffer": buffer,
                "width": width, "height": height, "x": MARGIN, "y": TOP, "span": span,
                "extent": header + height, "title": title, "metadata": metadata, "header": header}]})
        return pages
    board_width = max(WIDTH, math.ceil(max(item[2] for item in items)) + 2 * MARGIN)
    available = board_width - 2 * MARGIN
    pages, specimens = [], []
    x, y, row_height = MARGIN, TOP, 0
    for name, buffer, width, height in items:
        # Scenes are complete workspaces; each gets a full-width, unframed
        # stage. Smaller primitives pack at their natural widths in two or
        # three columns, with enough measure for their descriptive labels.
        span = min(available, max(256, math.ceil(width / 8) * 8))
        title, metadata, header = specimen_labels(name, span, width, height)
        specimen_height = header + height
        if x + span > board_width - MARGIN:
            x, y, row_height = MARGIN, y + row_height + 2 * GAP, 0
        if specimens and y + specimen_height + MARGIN + GAP > MAX_HEIGHT:
            pages.append({"width": board_width, "specimens": specimens})
            specimens = []
            x, y, row_height = MARGIN, TOP, 0
        specimens.append({"name": name, "buffer": buffer, "width": width, "height": height,
                          "x": x, "y": y, "span": span, "extent": specimen_height,
                          "title": title, "metadata": metadata, "header": header})
        x += span + GAP
        row_height = max(row_height, specimen_height)
    if specimens:
        pages.append({"width": board_width, "specimens": specimens})
    return pages


def atlas_frame(title, subtitle, width, height, colors, detail, description):
    root = ET.Element(f"{{{SVG}}}svg", {"width": f"{width:g}", "height": f"{height:g}",
        "viewBox": f"0 0 {width:g} {height:g}", "role": "img", "aria-labelledby": "title desc"})
    element(root, "title", {"id": "title"}, title + " — " + detail)
    element(root, "desc", {"id": "desc"}, subtitle + ". " + description)
    element(root, "rect", {"width": "100%", "height": "100%", "fill": colors["bg"]})
    element(root, "text", {"x": str(MARGIN), "y": "48", "fill": colors["fg"], "font-family": SANS,
        "font-size": str(TOKENS["typography"]["title_px"]), "font-weight": "600"}, title)
    element(root, "text", {"x": f"{width - MARGIN:g}", "y": "72", "fill": colors["muted"],
        "font-family": MONO, "font-size": str(TOKENS["typography"]["caption_px"]), "text-anchor": "end"}, detail)
    element(root, "path", {"d": f"M{MARGIN} 80H{width - MARGIN:g}", "stroke": colors["edge"], "stroke-width": "1"})
    return root


def paint_specimen(root, specimen, colors, center=False):
    name = specimen["name"]
    x, y, span = specimen["x"], specimen["y"], specimen["span"]
    section = element(root, "g", {"data-entry": name})
    for i, line in enumerate(specimen["title"]):
        element(section, "text", {"x": f"{x:g}", "y": f"{y + TOKENS['typography']['heading_px'] + i * GAP:g}",
            "fill": colors["fg"], "font-family": SANS, "font-size": str(TOKENS["typography"]["heading_px"]),
            "font-weight": "600"}, line)
    metadata_y = y + len(specimen["title"]) * GAP + TOKENS["typography"]["caption_px"]
    for i, line in enumerate(specimen["metadata"]):
        element(section, "text", {"x": f"{x:g}", "y": f"{metadata_y + i * TOKENS['spacing']['section']:g}",
            "fill": colors["muted"], "font-family": MONO, "font-size": str(TOKENS["typography"]["caption_px"])}, line)
    drawing = copy.deepcopy(specimen["buffer"])
    # Nested specimens retain their own accessible names without reusing the
    # board's title/description IDs or another specimen's IDs.
    identifiers = {node.attrib["id"]: f"{name}-{node.attrib['id']}"
                   for node in drawing.iter() if "id" in node.attrib}
    for node in drawing.iter():
        if "id" in node.attrib:
            node.attrib["id"] = identifiers[node.attrib["id"]]
        if "aria-labelledby" in node.attrib:
            node.attrib["aria-labelledby"] = " ".join(
                identifiers.get(value, value) for value in node.attrib["aria-labelledby"].split())
    drawing_x = x + (span - specimen["width"]) / 2 if center else x
    # Native viewport dimensions, and the buffer's own 16px text, survive
    # unchanged. There is no transform, faux terminal frame, or extra card.
    drawing.attrib.update({"x": f"{drawing_x:g}", "y": f"{y + specimen['header']:g}",
                           "width": f"{specimen['width']:g}", "height": f"{specimen['height']:g}"})
    section.append(drawing)


def board(group, profile, page, number, total):
    if group == "studio" and len(page["specimens"]) == 1 and page["specimens"][0]["name"] == "showcase-work":
        # The opening is the native terminal itself, without a second masthead.
        return ET.tostring(page["specimens"][0]["buffer"], encoding="unicode") + "\n"
    colors = palette(profile)
    title, subtitle = GROUPS[group]
    width = page["width"]
    specimens = page["specimens"]
    height = math.ceil(max(item["y"] + item["extent"] for item in specimens) + MARGIN + GAP)
    suffix = f" · page {number}/{total}" if total > 1 else ""
    detail = PROFILE_LABELS.get(profile, profile) + suffix
    root = atlas_frame(title, subtitle, width, height, colors, detail,
        "Actual ratatui buffer renders: " + ", ".join(item["name"] for item in specimens))
    row_starts = sorted({item["y"] for item in specimens})
    for y in row_starts[1:]:
        element(root, "path", {"d": f"M{MARGIN} {y - GAP:g}H{width - MARGIN:g}",
            "stroke": colors["edge"], "stroke-width": "1"})
    for item in specimens:
        paint_specimen(root, item, colors, group in {"whales", "whale-actions", "studio", "native-views", "scenes", "water"})
    return ET.tostring(root, encoding="unicode", xml_declaration=False) + "\n"


def profile_comparison(source):
    """The same native state-mark specimen in all nine terminal profiles."""
    colors = palette("dark-truecolor")
    buffers = [(profile, *load_buffer(source / f"status-marks.{profile}.svg")) for profile in PROFILES]
    width = max(WIDTH, 2 * MARGIN + 3 * math.ceil(max(item[2] for item in buffers)) + 2 * GAP)
    column = (width - 2 * MARGIN - 2 * GAP) / 3
    specimens = []
    for profile, buffer, body_width, body_height in buffers:
        title = label_lines(PROFILE_LABELS[profile], column, TOKENS["typography"]["heading_px"])
        metadata = [profile]
        header = len(title) * GAP + TOKENS["spacing"]["section"] * 2
        specimens.append({"name": "status-marks", "buffer": buffer, "width": body_width, "height": body_height,
                          "span": column, "title": title, "metadata": metadata, "header": header,
                          "extent": header + body_height})
    row_height = max(item["extent"] for item in specimens)
    height = TOP + 3 * row_height + 4 * GAP + MARGIN
    root = atlas_frame("One language, every terminal", "State stays legible across color depths, measured grounds and plain text",
        width, height, colors, "9 terminal profiles", "Actual status-mark buffers: " + ", ".join(PROFILES))
    for i, specimen in enumerate(specimens):
        specimen.update({"x": MARGIN + (i % 3) * (column + GAP), "y": TOP + (i // 3) * (row_height + GAP)})
        paint_specimen(root, specimen, colors)
    return ET.tostring(root, encoding="unicode", xml_declaration=False) + "\n"


def generate(source, profiles):
    names, exported = read_inputs(source, profiles)
    outputs, index = {}, []
    for profile in profiles:
        grouped = {group: [] for group in GROUPS}
        for name in names:
            buffer, width, height = load_buffer(exported[profile][name])
            grouped[classify(name)].append((name, buffer, width, height))
        represented = []
        for group, items in grouped.items():
            pages = layout(items, group)
            for number, page in enumerate(pages, 1):
                suffix = f"-{number}" if len(pages) > 1 else ""
                filename = f"{group}.{profile}{suffix}.svg"
                outputs[filename] = board(group, profile, page, number, len(pages))
                page_names = [item["name"] for item in page["specimens"]]
                represented.extend(page_names)
                index.append({"file": filename, "profile": profile, "group": group, "entries": page_names})
        if sorted(represented) != sorted(names):
            raise ValueError(f"{profile}: gallery coverage mismatch")
    outputs["profile-comparison.svg"] = profile_comparison(source)
    index.append({"file": "profile-comparison.svg", "profile": "all", "group": "profiles",
                  "entries": ["status-marks"], "profiles": PROFILES})
    outputs["index.json"] = json.dumps({"entries": names, "boards": index}, indent=2) + "\n"
    return outputs, len(names), index


def readme_gallery(readme, destination, index):
    """Replace the bounded generated slot while preserving all authored text."""
    original = readme.read_text(encoding="utf-8")
    start, end = "<!-- gallery:start -->", "<!-- gallery:end -->"
    if original.count(start) != 1 or original.count(end) != 1:
        raise ValueError(f"{readme}: expected exactly one gallery:start / gallery:end marker pair")
    begin, finish = original.index(start), original.index(end)
    if finish < begin:
        raise ValueError(f"{readme}: gallery markers are reversed")

    def embed(item):
        title = ", ".join(specimen_title(name) for name in item["entries"])
        profile = item["profile"].replace("-", " ")
        alt = f"{title} — {profile}" if item["profile"] != "all" else "The same status marks in all nine terminal profiles"
        path = Path(os.path.relpath(destination / item["file"], readme.parent)).as_posix()
        return f"![{alt}](<{path}>)"

    primary = [item for item in index
               if item["profile"] != "all" and not item["profile"].startswith("light")]
    families = list(dict.fromkeys(item["group"] for item in primary))
    sections = {
        "Native Codewhale": ["studio", "pet", "native-views", "native-chrome", "workbar"],
        "Color and atmosphere": ["tui-palettes", "water"],
        "Inputs and controls": ["foundation", "input", "chrome"],
        "Conversation and work": ["transcript", "components", "display", "scenes"],
        "Motion and marine life": ["motion", "habitat", "whales", "whale-actions"],
    }
    if set(families) != {family for group in sections.values() for family in group}:
        raise ValueError("README sections must cover every gallery family")
    lines = [start, ""]
    for heading, groups in sections.items():
        lines.extend(["### " + heading, ""])
        for family in groups:
            title, description = GROUPS[family]
            anchor = re.sub(r"[^a-z0-9]+", "-", title.lower()).strip("-")
            boards = [item for item in primary if item["group"] == family]
            count = len({entry for item in boards for entry in item["entries"]})
            quantity = f"{count} example" + ("s" if count != 1 else "")
            lines.extend([f'<a id="{anchor}"></a>', "<details>",
                          f"<summary>{title} · {quantity}</summary>", "", description + ".", ""])
            lines.extend(embed(item) + "\n" for item in boards)
            light = [item for item in index if item["group"] == family and item["profile"].startswith("light")]
            if light:
                lines.extend(["<details>", "<summary>Light appearance</summary>", ""])
                lines.extend(embed(item) + "\n" for item in light)
                lines.extend(["</details>", ""])
            movies = {
                "pet": [("pet-dark.gif", "The whale's cove and activity tour — dark"),
                        ("pet-light.gif", "The same animated whale — light"),
                        ("pet-mode-dark.gif", "Full pet surface at rest — dark"),
                        ("pet-mode-light.gif", "Full pet surface at rest — light")],
                "studio": [("showcase.gif", "Native work, approval and completion"),
                           ("showcase-light.gif", "The same native layout in WhaleLight")],
                "motion": [("motion-demo.gif", "Working and verification spinners — Ocean"),
                           ("motion-demo-light.gif", "Working and verification spinners — Paper")],
                "habitat": [("habitat-motion.gif", "Native fish, jellyfish and bubbles")],
                "whale-actions": [("whale-performance.gif", "All seventeen native whale actions")],
            }.get(family, [])
            if movies:
                lines.extend(["<details>", "<summary>Watch the animation</summary>", ""])
                for filename, alt in movies:
                    path = Path(os.path.relpath(destination / filename, readme.parent)).as_posix()
                    lines.extend([f"![{alt}](<{path}>)", ""])
                lines.extend(["</details>", ""])
            lines.extend(["</details>", ""])
    comparison = [item for item in index if item["file"] == "profile-comparison.svg"]
    if comparison:
        lines.extend(['<a id="terminal-profiles"></a>', "<details>",
                      "<summary>All nine terminal profiles</summary>", ""])
        lines.extend(embed(item) + "\n" for item in comparison)
        lines.extend(["</details>", ""])
    lines.append(end)
    generated = "\n".join(lines)
    return original[:begin] + generated + original[finish + len(end):]


def previous_owned_files(destination):
    """Only the previous index can authorize removing an obsolete board."""
    path = destination / "index.json"
    if not path.exists():
        return set()
    previous = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(previous, dict) or not isinstance(previous.get("boards"), list):
        raise ValueError(f"{path}: expected a generated board index")
    names = set()
    for board in previous["boards"]:
        name = board.get("file") if isinstance(board, dict) else None
        # Strict basenames also reject Windows separators/drives, hidden files,
        # parent traversal and non-SVG artifacts. Never recurse or glob-delete.
        if not isinstance(name, str) or not re.fullmatch(r"[a-z0-9][a-z0-9.-]*\.svg", name) or ".." in name:
            raise ValueError(f"{path}: unsafe generated board basename {name!r}")
        names.add(name)
    return names


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("source", type=Path, help="directory written by gallery --svg")
    parser.add_argument("destination", type=Path, help="generated README asset directory")
    parser.add_argument("--profiles", nargs="+", default=["dark-truecolor", "light-truecolor"])
    parser.add_argument("--check", action="store_true", help="compare generated bytes without writing")
    parser.add_argument("--readme", type=Path, help="update the gallery:start / gallery:end slot in this README")
    args = parser.parse_args()
    if args.source.resolve() == args.destination.resolve():
        parser.error("source and destination must be separate directories; use target/readme-buffers and assets/readme")
    if len(set(args.profiles)) != len(args.profiles):
        parser.error("duplicate profiles")
    try:
        outputs, count, index = generate(args.source, args.profiles)
        readme = readme_gallery(args.readme, args.destination, index) if args.readme else None
        obsolete = previous_owned_files(args.destination) - outputs.keys()
        obsolete = sorted(name for name in obsolete
                          if (args.destination / name).exists() or (args.destination / name).is_symlink())
        if args.check:
            stale = [name for name, content in outputs.items()
                     if not (args.destination / name).exists()
                     or (args.destination / name).read_text(encoding="utf-8") != content]
            if readme is not None and args.readme.read_text(encoding="utf-8") != readme:
                stale.append(f"{args.readme} gallery slot")
            stale.extend(f"obsolete indexed board {name}" for name in obsolete)
            if stale:
                raise ValueError("stale or missing assets: " + ", ".join(stale))
            print(f"Verified {len(index)} boards covering all {count} entries × {len(args.profiles)} profiles")
        else:
            args.destination.mkdir(parents=True, exist_ok=True)
            for name in obsolete:
                (args.destination / name).unlink()
            for name, content in outputs.items():
                (args.destination / name).write_bytes(content.encode("utf-8"))
            if readme is not None:
                args.readme.write_bytes(readme.encode("utf-8"))
            print(f"Wrote {len(index)} boards covering all {count} entries × {len(args.profiles)} profiles")
    except (OSError, ValueError, KeyError, ET.ParseError) as error:
        print(f"render-gallery: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
