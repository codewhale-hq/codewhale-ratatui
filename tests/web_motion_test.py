"""Native web motion integrity and read-only staleness checks.

Run with: python3 -B tests/web_motion_test.py
"""

import contextlib
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("web_motion", ROOT / "tools/export-web-motion.py")
EXPORT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(EXPORT)


def specimen(root, profile="dark-truecolor", interval=100):
    """Small synthetic fixture for the existing manifest trust boundaries."""
    source = root / "frames"
    source.mkdir()
    frames = [{"file": f"frame-{index:03}.svg", "elapsed_ms": 6000 + index * interval,
               "profile": profile, "width": 4, "height": 2} for index in range(2)]
    for index, frame in enumerate(frames):
        (source / frame["file"]).write_text(
            '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 40">'
            f'<text x="0" y="20">🐳 &amp; {index}</text></svg>\n', encoding="utf-8",
        )
    manifest = json.dumps(frames).encode()
    (source / "manifest.json").write_bytes(manifest)
    digest = hashlib.sha256(manifest)
    for frame in frames:
        digest.update(frame["file"].encode() + b"\0")
        digest.update((source / frame["file"]).read_bytes())
    gif = root / "specimen.gif"
    gif.write_bytes(b"test record")
    record = {"source_sha256": digest.hexdigest(), "gif_sha256": hashlib.sha256(gif.read_bytes()).hexdigest(),
              "frames": 2, "width": 40, "height": 40, "frame_ms": interval}
    Path(str(gif) + ".json").write_text(json.dumps(record), encoding="utf-8")
    return {"id": "test", "title": "Test", "profile": "dark-truecolor", "frames": "frames",
            "gif": "specimen.gif", "example": "examples/showcase.rs"}


class WebMotionTests(unittest.TestCase):
    def export(self, track, root):
        with contextlib.redirect_stdout(io.StringIO()):
            return EXPORT.export_track(track, root, "a" * 40, EXPORT.animation_checker())

    def test_preserves_native_strings_cell_geometry_interval_and_source_revision(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            track = specimen(root)
            payload, record = self.export(track, root)
            value = json.loads(payload)
            self.assertEqual(value["width"], 4)
            self.assertEqual(value["height"], 2)
            self.assertEqual(value["frameMs"], 100)
            self.assertEqual(record["startMs"], 6000)
            self.assertEqual(value["frames"][1], (root / "frames/frame-001.svg").read_text())
            self.assertIn("/blob/" + "a" * 40 + "/examples/showcase.rs", value["source"])
            self.assertEqual(self.export(track, root)[0], payload)

    def test_stale_frame_and_gif_record_are_rejected_before_export(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            track = specimen(root)
            frame = root / "frames/frame-000.svg"
            frame.write_text(frame.read_text().replace("&amp;", "&lt;"))
            with self.assertRaisesRegex(ValueError, "stale"):
                self.export(track, root)

    def test_manifest_cannot_escape_its_frame_directory(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            track = specimen(root)
            path = root / "frames/manifest.json"
            manifest = json.loads(path.read_text())
            manifest[0]["file"] = "../specimen.gif"
            path.write_text(json.dumps(manifest))
            with self.assertRaisesRegex(ValueError, "manifest"):
                self.export(track, root)

    def test_invalid_timing_and_mislabeled_profile_fail_even_with_matching_hashes(self):
        for profile, interval, error in (("dark-truecolor", 10, "interval"),
                                         ("light-truecolor", 100, "profile"),
                                         ("invented", 100, "profile")):
            with self.subTest(profile=profile, interval=interval), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                track = specimen(root, profile, interval)
                with self.assertRaisesRegex(ValueError, error):
                    self.export(track, root)

    def test_active_svg_and_external_paints_are_rejected(self):
        valid = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 40"><text>x</text></svg>'
        for changed in (valid.replace("<text>", '<text onclick="alert(1)">'),
                        valid.replace("</svg>", '<script>alert(1)</script></svg>'),
                        valid.replace("<text>", '<text fill="url(https://example.com/paint)">'),
                        '<!DOCTYPE svg [<!ENTITY remote SYSTEM "file:///tmp/private">]>' + valid):
            with self.subTest(svg=changed), self.assertRaisesRegex(ValueError, "unsupported"):
                EXPORT.validate_svg(changed, 4, 2)
        with self.assertRaisesRegex(ValueError, "geometry"):
            EXPORT.validate_svg(valid, 5, 2)

    def test_asset_limit_is_checked_before_any_output_is_written(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            track = specimen(root)
            with patch.object(EXPORT, "MAX_ASSET_BYTES", 1), self.assertRaisesRegex(ValueError, "exceeds"):
                self.export(track, root)

    def test_check_never_writes_and_export_preserves_neighbors(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            neighbor = root / "other.json"
            neighbor.write_text("someone else's data")
            output = {"studio.json": b"native frames\n"}
            with self.assertRaisesRegex(ValueError, "stale"):
                EXPORT.write(output, root, check=True)
            self.assertFalse((root / "studio.json").exists())
            EXPORT.write(output, root)
            EXPORT.write(output, root, check=True)
            (root / "studio.json").write_text("stale")
            with self.assertRaisesRegex(ValueError, "stale"):
                EXPORT.write(output, root, check=True)
            self.assertEqual((root / "studio.json").read_text(), "stale")
            self.assertEqual(neighbor.read_text(), "someone else's data")


if __name__ == "__main__":
    unittest.main()
