#!/usr/bin/env python3
"""Publish the real-buffer website catalogue into any static public directory.

    cargo run --locked --example website -- target/website-buffers
    python3 tools/export-website.py target/website-buffers ../CodeWhale/web/public/ratatui
    python3 tools/export-website.py target/website-buffers ../CodeWhale/web/public/ratatui --check

Only catalogue.json and entries named by its previous manifest are owned by this
exporter. No network, Node dependencies, timestamps or fabricated Rust examples.
"""

import argparse
from collections import Counter
import fnmatch
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
SIZES = [{"id": "native", "label": "Native"}, *[
    {"id": str(width), "label": f"{width} columns"} for width in (40, 80, 120)
]]
SAFE_NAME = re.compile(r"[a-zA-Z0-9_-]+\Z")


def read_renderer():
    """Reuse the README's single family and terminal-profile authority."""
    spec = importlib.util.spec_from_file_location("readme_gallery", ROOT / "tools/render-gallery.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def canonical_json(value):
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")) + "\n").encode()


def mask_rust(source):
    """Keep positions while hiding quoted text and nested comments for extraction."""
    out = list(source)
    index = 0
    while index < len(source):
        end = None
        if source.startswith("//", index):
            end = source.find("\n", index)
            if end < 0:
                end = len(source)
        elif source.startswith("/*", index):
            cursor, depth = index + 2, 1
            while cursor < len(source) and depth:
                if source.startswith("/*", cursor):
                    depth += 1
                    cursor += 2
                elif source.startswith("*/", cursor):
                    depth -= 1
                    cursor += 2
                else:
                    cursor += 1
            end = cursor
        else:
            raw = re.match(r'(?:b)?r(#+)?"', source[index:])
            if raw and (index == 0 or not source[index - 1].isalnum()):
                terminator = '"' + (raw.group(1) or "")
                close = source.find(terminator, index + raw.end())
                end = len(source) if close < 0 else close + len(terminator)
            elif source[index] == '"':
                cursor = index + 1
                while cursor < len(source):
                    if source[cursor] == "\\":
                        cursor += 2
                    elif source[cursor] == '"':
                        cursor += 1
                        break
                    else:
                        cursor += 1
                end = cursor
            elif source[index] == "'":
                char = re.match(r"'(?:\\(?:u\{[0-9a-fA-F_]+\}|x[0-9a-fA-F]{2}|.)|[^'\\\n])'", source[index:])
                if char:
                    end = index + char.end()
        if end is None:
            index += 1
        else:
            for pos in range(index, min(end, len(source))):
                if out[pos] != "\n":
                    out[pos] = " "
            index = end
    return "".join(out)


def balanced_end(masked, start):
    depth = 0
    for pos in range(start, len(masked)):
        if masked[pos] == "{":
            depth += 1
        elif masked[pos] == "}":
            depth -= 1
            if not depth:
                return pos + 1
    raise ValueError("unclosed Rust body in gallery source")


def functions(source):
    masked = mask_rust(source)
    output = {}
    pattern = re.compile(r"(?m)^((?:pub(?:\([^)]*\))?\s+)?fn\s+([a-zA-Z_][\w]*)\s*(?:<[^\n]*>)?\s*\()")
    # Only column-zero free functions; methods are not private fixture helpers.
    for match in pattern.finditer(masked):
        start = match.start()
        body = masked.find("{", match.end())
        if body < 0:
            raise ValueError("missing Rust function body")
        end = balanced_end(masked, body)
        output[match.group(2)] = {
            "code": source[start:end], "line": source.count("\n", 0, start) + 1,
        }
    return output


def imports(source):
    masked = mask_rust(source)
    output = []
    for match in re.finditer(r"(?m)^use\s+", masked):
        end = masked.find(";", match.end())
        text = source[match.start():end + 1]
        if text.startswith("use super::"):
            continue
        output.append(text.replace("use crate::", "use codewhale_ratatui::", 1))
    return "\n\n".join(output)


def gallery_sources(root=ROOT):
    """Map literal, tuple, macro and closure fixtures without a second entry list."""
    output = {}
    for path in sorted((root / "src/gallery").glob("*.rs")):
        source = path.read_text(encoding="utf-8")
        funcs = functions(source)
        records = []
        for match in re.finditer(r'Entry\s*\{\s*name:\s*"([^"]+)"', source):
            end = balanced_end(mask_rust(source), source.find("{", match.start()))
            body = source[match.start():end]
            draw = re.search(r"draw:\s*([^\n]+)", body)
            if draw is None:
                raise ValueError(f"missing draw in {path}:{match.group(1)}")
            expression = draw.group(1).rstrip().removesuffix(",")
            function = re.match(r"([a-zA-Z_]\w*)\s*$", expression)
            records.append((match.group(1), function.group(1) if function else None, match.start(), body, expression))
        # Four-column tuples and two-column tuples map into Entry in the source.
        entry_code = funcs.get("entries", {}).get("code", "")
        entry_start = source.find(entry_code) if entry_code else 0
        for match in re.finditer(r'\(\s*"([a-zA-Z0-9_-]+)"\s*,\s*(?:\d+\s*,\s*\d+\s*,\s*)?([a-zA-Z_]\w*)\s*(?:as\s+fn\([^\n]*\))?\s*,?\s*\)', entry_code):
            records.append((match.group(1), match.group(2), entry_start + match.start(), match.group(0), match.group(2)))
        for name, function, position, literal, expression in records:
            if name in output:
                raise ValueError(f"duplicate gallery source mapping for {name}")
            line = source.count("\n", 0, position) + 1
            helper_codes = []
            if function in funcs:
                primary = funcs[function]
                code, line = primary["code"], primary["line"]
                pending, seen = [code], {function}
                while pending:
                    item = mask_rust(pending.pop())
                    for called in re.findall(r"(?<![.\w:])([a-zA-Z_]\w*)\s*\(", item):
                        if called in funcs and called not in seen:
                            seen.add(called)
                            helper = funcs[called]
                            helper_codes.append({"name": called, **helper})
                            pending.append(helper["code"])
            else:
                macro = re.search(rf"(?m)^palette_draw!\({re.escape(function or '')},\s*\w+\);", source)
                if macro:
                    macro_start = source.index("macro_rules! palette_draw")
                    macro_end = balanced_end(mask_rust(source), source.index("{", macro_start))
                    code = source[macro_start:macro_end] + "\n" + macro.group(0)
                    line = source.count("\n", 0, macro.start()) + 1
                    helper_codes = [{"name": "sample", **funcs["sample"]}]
                else:
                    # Closures are retained verbatim, along with their fixture helper.
                    code = literal
                    for called in re.findall(r"\b([a-zA-Z_]\w*)\s*\(", expression):
                        if called in funcs:
                            helper_codes.append({"name": called, **funcs[called]})
            # Macro/closure fixture excerpts need their helpers' helpers too.
            pending = [helper["code"] for helper in helper_codes]
            seen = {function, *[helper["name"] for helper in helper_codes]}
            while pending:
                item = mask_rust(pending.pop())
                for called in re.findall(r"(?<![.\w:])([a-zA-Z_]\w*)\s*\(", item):
                    if called in funcs and called not in seen:
                        seen.add(called)
                        helper = funcs[called]
                        helper_codes.append({"name": called, **helper})
                        pending.append(helper["code"])
            relative = path.relative_to(root).as_posix()
            output[name] = {
                "file": relative, "function": function or "closure", "line": line,
                "code": code, "imports": imports(source), "helpers": helper_codes,
            }
    return output


def crosswalk(root=ROOT):
    rows = []
    for line in (root / "COMPONENTS.md").read_text(encoding="utf-8").splitlines():
        if not line.startswith("| "):
            continue
        cells = [cell.strip() for cell in line.strip("|").split("|")]
        if len(cells) != 4 or cells[2] in ("Kind", "---"):
            continue
        symbols = re.findall(r"`([^`]+)`", cells[1])
        patterns = re.findall(r"`([^`]+)`", cells[3])
        if symbols and patterns:
            rows.append({"symbols": symbols, "patterns": patterns, "kind": cells[2]})
    return rows


def public_symbols(mapping, docs):
    """Responsive recipes still expose the public types their real source uses."""
    names = set(re.findall(r"\b[A-Z][a-zA-Z0-9_]*\b", " ".join(
        symbol for row in docs for symbol in row["symbols"]
    )))
    code = mask_rust(mapping["code"] + "\n" + "\n".join(helper["code"] for helper in mapping["helpers"]))
    return sorted(name for name in names if re.search(rf"(?<!:)\b{re.escape(name)}\b", code))


def provenance(root=ROOT):
    paths = ["src", "assets/whale-v2.scenes", "assets/whale-motion", "assets/tui-palettes.json",
             "assets/tui-source.json", "vendor/codewhale-design", "COMPONENTS.md", "VIEWS.md"]
    revision = subprocess.check_output(
        ["git", "log", "-1", "--format=%H", "--", *paths], cwd=root, text=True,
    ).strip()
    digest = hashlib.sha256()
    for relative in paths:
        path = root / relative
        files = sorted(path.rglob("*")) if path.is_dir() else [path]
        for item in files:
            if item.is_file():
                digest.update(item.relative_to(root).as_posix().encode() + b"\0" + item.read_bytes() + b"\0")
    native = json.loads((root / "assets/tui-source.json").read_text(encoding="utf-8"))
    return {"revision": revision, "digest": digest.hexdigest(), "repository": REPOSITORY,
            "nativeRevision": native["commit"], "nativeRepository": native["repository"]}


def validate_entry(value, descriptor, profiles):
    name = descriptor["name"]
    if not SAFE_NAME.fullmatch(name):
        raise ValueError(f"invalid gallery name {name!r}")
    if any(value.get(key) != descriptor[key] for key in ("name", "width", "height")):
        raise ValueError(f"{name}: preview dimensions/name differ from manifest")
    previews = value.get("previews", {})
    if set(previews) != set(profiles):
        raise ValueError(f"{name}: missing or extra terminal profiles")
    for profile, sizes in previews.items():
        if set(sizes) != {size["id"] for size in SIZES}:
            raise ValueError(f"{name}/{profile}: missing or extra width choices")
        for size, svg in sizes.items():
            element = ET.fromstring(svg)
            cols = descriptor["width"] if size == "native" else int(size)
            expected = f"0 0 {max(1, cols * 10)} {max(1, descriptor['height'] * 20)}"
            if element.tag != "{http://www.w3.org/2000/svg}svg" or element.get("viewBox") != expected:
                raise ValueError(f"{name}/{profile}/{size}: SVG geometry does not match the buffer")
            for node in element.iter():
                tag = node.tag.rsplit("}", 1)[-1]
                if tag in {"script", "foreignObject", "image", "iframe"}:
                    raise ValueError(f"{name}: unsupported active/external SVG element {tag}")
                if any(key.lower().startswith("on") or key.rsplit("}", 1)[-1] == "href" for key in node.attrib):
                    raise ValueError(f"{name}: unsupported active/external SVG attribute")


def generate(source, root=ROOT):
    renderer = read_renderer()
    manifest = json.loads((source / "manifest.json").read_text(encoding="utf-8"))
    if not manifest or len({entry["name"] for entry in manifest}) != len(manifest):
        raise ValueError("gallery manifest must contain unique entries")
    mappings, docs, origin = gallery_sources(root), crosswalk(root), provenance(root)
    output, entries = {}, []
    for descriptor in manifest:
        name = descriptor["name"]
        if not SAFE_NAME.fullmatch(name):
            raise ValueError(f"invalid gallery name {name!r}")
        if name not in mappings:
            raise ValueError(f"no exact Rust fixture mapping for {name}")
        value = json.loads((source / f"{name}.json").read_text(encoding="utf-8"))
        validate_entry(value, descriptor, renderer.PROFILES)
        mapping = mappings[name]
        fixture_source = {key: mapping[key] for key in ("file", "function", "line")}
        fixture_source["url"] = f"{REPOSITORY}/blob/{origin['revision']}/{mapping['file']}#L{mapping['line']}"
        family = renderer.classify(name)
        matches = [row for row in docs if any(fnmatch.fnmatchcase(name, pattern) for pattern in row["patterns"])]
        symbols = public_symbols(mapping, docs) or list(dict.fromkeys(
            symbol for row in matches for symbol in row["symbols"]))
        fixture = {key: mapping[key] for key in ("code", "imports", "helpers")}
        fixture["source"] = fixture_source
        fixture["kind"] = "source-fixture"
        fixture["standalone"] = False
        value["fixture"] = fixture
        preview_path = f"entries/{name}.json"
        output[preview_path] = canonical_json(value)
        entries.append({**descriptor, "title": name.replace("-", " ").capitalize(), "family": family,
                        "description": renderer.GROUPS[family][1], "api": symbols,
                        "kind": matches[0]["kind"] if matches else "Gallery composition",
                        "source": fixture_source, "previewPath": preview_path})
    counts = Counter(entry["family"] for entry in entries)
    catalogue = {"schemaVersion": 1, "source": origin, "count": len(entries),
                 "profiles": [{"id": profile, "label": renderer.PROFILE_LABELS[profile]} for profile in renderer.PROFILES],
                 "sizes": SIZES, "families": [{"id": family, "title": title, "description": description,
                 "count": counts[family]} for family, (title, description) in renderer.GROUPS.items() if counts[family]],
                 "entries": entries}
    output["catalogue.json"] = canonical_json(catalogue)
    return output, catalogue


def write_outputs(destination, outputs, check=False):
    failures = []
    old = destination / "catalogue.json"
    owned = {"catalogue.json"}
    if old.exists():
        previous = json.loads(old.read_text(encoding="utf-8"))
        for entry in previous.get("entries", []):
            name = entry.get("name", "")
            if not SAFE_NAME.fullmatch(name):
                raise ValueError("unsafe name in previous website catalogue")
            owned.add(f"entries/{name}.json")
    for relative, content in outputs.items():
        path = destination / relative
        if check:
            if not path.exists() or path.read_bytes() != content:
                failures.append(relative)
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(content)
    for relative in sorted(owned - outputs.keys()):
        path = destination / relative
        if path.exists():
            if check:
                failures.append(relative)
            else:
                path.unlink()
    if failures:
        raise ValueError("website exports differ: " + ", ".join(failures))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path, help="directory written by the website Rust example")
    parser.add_argument("destination", type=Path, help="public catalogue directory in the website")
    parser.add_argument("--check", action="store_true", help="verify exported files without changing them")
    args = parser.parse_args()
    try:
        outputs, catalogue = generate(args.source)
        write_outputs(args.destination, outputs, args.check)
    except (OSError, ValueError, KeyError, ET.ParseError, subprocess.SubprocessError) as error:
        print(f"Website export failed: {error}", file=sys.stderr)
        return 1
    print(f"{'Verified' if args.check else 'Wrote'} {catalogue['count']} entries × {len(catalogue['profiles'])} profiles × {len(SIZES)} widths")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
