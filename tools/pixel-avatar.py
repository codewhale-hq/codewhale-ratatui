#!/usr/bin/env python3
"""Terminal tiles and a hash manifest for a hand-drawn pixel-art avatar pack.

    python3 tools/pixel-avatar.py assets/pixel-whale --cell 8 --grid whale.grid:<sha256>
    python3 tools/pixel-avatar.py assets/pixel-whale --cell 8 --grid whale.grid:<sha256> --check

A pixel-art page is drawn with one art cell as a square of `--cell` page
pixels. This reads avatar.json and its PNG pages, refuses a page where any
cell is not one flat colour, and writes terminal.rgba (one RGBA pixel per art
cell, tiles in manifest order) and manifest.json beside them. `--check` writes
nothing and fails when either file is stale. `--grid` records the name and
SHA-256 of the hand-written grid the pages were drawn from. Needs Pillow.
"""

import argparse
import hashlib
import json
from pathlib import Path
import sys

from PIL import Image


def build(directory: Path, cell: int, grid: str | None) -> tuple[bytes, str]:
    pack = json.loads((directory / "avatar.json").read_text(encoding="utf-8"))
    width, height = pack["tileWidth"], pack["tileHeight"]
    if width % cell or height % cell:
        sys.exit(f"{directory}: a {width}x{height} tile is not a whole number of {cell}px cells")
    tiles, pages = bytearray(), []
    for name in pack["atlases"]:
        data = (directory / name).read_bytes()
        page = Image.open(directory / name).convert("RGBA")
        if page.size != (width * pack["columns"], height * pack["rows"]):
            sys.exit(f"{directory / name}: {page.size} does not match the manifest")
        # Nearest and box agree only when every cell is one flat colour.
        small = (page.width // cell, page.height // cell)
        cells = page.resize(small, Image.NEAREST)
        if cells.resize(page.size, Image.NEAREST).tobytes() != page.tobytes():
            sys.exit(f"{directory / name}: not pixel art on a {cell}px cell grid")
        for index in range(pack["columns"] * pack["rows"]):
            x = index % pack["columns"] * (width // cell)
            y = index // pack["columns"] * (height // cell)
            tiles += cells.crop((x, y, x + width // cell, y + height // cell)).tobytes()
        pages.append({"file": name, "sha256": hashlib.sha256(data).hexdigest()})
    manifest = {
        "version": 1,
        "pixelArt": True,
        "cell": cell,
        "tile": [width, height],
        "terminalTile": [width // cell, height // cell],
        "frames": pack["columns"] * pack["rows"] * len(pack["atlases"]),
        "packSha256": hashlib.sha256((directory / "avatar.json").read_bytes()).hexdigest(),
        "pages": pages,
        "terminalSha256": hashlib.sha256(tiles).hexdigest(),
    }
    if grid:
        name, _, digest = grid.partition(":")
        manifest["source"] = {"grid": name, "sha256": digest}
    return bytes(tiles), json.dumps(manifest, indent=2) + "\n"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("directory", type=Path)
    parser.add_argument("--cell", type=int, required=True)
    parser.add_argument("--grid")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if args.cell < 1:
        sys.exit("--cell must be at least 1")
    tiles, manifest = build(args.directory, args.cell, args.grid)
    terminal, record = args.directory / "terminal.rgba", args.directory / "manifest.json"
    if args.check:
        stale = [
            str(path)
            for path, want in ((terminal, tiles), (record, manifest.encode()))
            if not path.exists() or path.read_bytes() != want
        ]
        if stale:
            sys.exit("stale: " + ", ".join(stale))
        return
    terminal.write_bytes(tiles)
    record.write_text(manifest, encoding="utf-8")
    print(f"{terminal} {len(tiles)} bytes")


if __name__ == "__main__":
    main()
