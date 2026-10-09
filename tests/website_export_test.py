"""Website export contracts: real dimensions, exact source and scoped ownership.

Run with: python3 -B tests/website_export_test.py
"""

import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("website_export", ROOT / "tools/export-website.py")
EXPORT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(EXPORT)


def specimen():
    descriptor = {"name": "native-composer", "width": 100, "height": 4}
    value = {**descriptor, "previews": {}}
    for profile in EXPORT.read_renderer().PROFILES:
        value["previews"][profile] = {}
        for size in EXPORT.SIZES:
            key = size["id"]
            columns = descriptor["width"] if key == "native" else int(key)
            value["previews"][profile][key] = (
                f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {columns * 10} 80">'
                '<title>Real buffer geometry</title><text>Enter send</text></svg>'
            )
    return descriptor, value


class WebsiteExportTests(unittest.TestCase):
    def test_width_choice_is_a_redraw_not_a_scaled_native_image(self):
        descriptor, value = specimen()
        profiles = EXPORT.read_renderer().PROFILES
        EXPORT.validate_entry(value, descriptor, profiles)
        value["previews"][profiles[0]]["40"] = value["previews"][profiles[0]]["native"]
        with self.assertRaisesRegex(ValueError, "geometry"):
            EXPORT.validate_entry(value, descriptor, profiles)

    def test_missing_fallback_and_extra_width_are_rejected(self):
        descriptor, value = specimen()
        profiles = EXPORT.read_renderer().PROFILES
        missing = copy.deepcopy(value)
        del missing["previews"]["ascii"]
        with self.assertRaisesRegex(ValueError, "profiles"):
            EXPORT.validate_entry(missing, descriptor, profiles)
        value["previews"][profiles[0]]["200"] = "ignored"
        with self.assertRaisesRegex(ValueError, "width choices"):
            EXPORT.validate_entry(value, descriptor, profiles)

    def test_active_svg_and_external_links_are_rejected(self):
        descriptor, value = specimen()
        profiles = EXPORT.read_renderer().PROFILES
        svg = value["previews"][profiles[0]]["native"]
        for content in ('<script>alert(1)</script>', '<text onclick="alert(1)">x</text>',
                        '<a href="https://example.com"><text>x</text></a>', '<image/>'):
            value["previews"][profiles[0]]["native"] = svg.replace("</svg>", content + "</svg>")
            with self.assertRaisesRegex(ValueError, "unsupported"):
                EXPORT.validate_entry(value, descriptor, profiles)

    def test_function_extract_keeps_real_quoted_braces_and_ignores_comments(self):
        source = '''fn paint(area: Rect) {
    let ordinary = "} escaped \\\" {";
    let raw = r##"} {"##;
    let character = '}';
    // } ignored
    /* { nested /* } */ comment */
    helper(area);
}
fn helper(area: Rect) { consume(area); }
'''
        extracted = EXPORT.functions(source)
        self.assertEqual(set(extracted), {"paint", "helper"})
        self.assertTrue(extracted["paint"]["code"].endswith("helper(area);\n}"))
        self.assertIn('let raw = r##"} {"##;', extracted["paint"]["code"])

    def test_catalogue_retains_literal_tuple_macro_and_closure_fixtures(self):
        mappings = EXPORT.gallery_sources()
        for name, function in [("native-composer", "composer"), ("workbar-left", "left"),
                               ("tui-theme-underwater", "underwater"), ("showcase-work", "closure")]:
            mapping = mappings[name]
            self.assertEqual(mapping["function"], function)
            original = (ROOT / mapping["file"]).read_text(encoding="utf-8")
            self.assertTrue(mapping["code"] in original or name.startswith("tui-theme-"))
            self.assertGreater(mapping["line"], 0)
        self.assertIn("fn side_with(", "\n".join(
            helper["code"] for helper in mappings["workbar-left"]["helpers"]
        ))
        self.assertIn("fn band(", "\n".join(
            helper["code"] for helper in mappings["tui-theme-underwater"]["helpers"]
        ))

    def test_enum_variant_names_are_not_mistaken_for_public_components(self):
        mappings = EXPORT.gallery_sources()
        symbols = EXPORT.public_symbols(mappings["workbar-left"], EXPORT.crosswalk())
        self.assertIn("Workbar", symbols)
        self.assertNotIn("Fleet", symbols)  # WorkbarPanel::Fleet is an enum variant.
        self.assertNotIn("Cost", symbols)

    def test_check_catches_tamper_and_cleanup_preserves_unowned_files(self):
        with tempfile.TemporaryDirectory() as directory:
            destination = Path(directory)
            previous = {"entries": [{"name": "old-widget"}]}
            (destination / "entries").mkdir()
            (destination / "catalogue.json").write_text(json.dumps(previous))
            (destination / "entries/old-widget.json").write_text("old")
            (destination / "entries/my-manual-notes.json").write_text("keep")
            outputs = {"catalogue.json": EXPORT.canonical_json({"entries": [{"name": "new-widget"}]}),
                       "entries/new-widget.json": b'{"name":"new-widget"}\n'}
            with self.assertRaisesRegex(ValueError, "differ"):
                EXPORT.write_outputs(destination, outputs, check=True)
            EXPORT.write_outputs(destination, outputs)
            self.assertFalse((destination / "entries/old-widget.json").exists())
            self.assertEqual((destination / "entries/my-manual-notes.json").read_text(), "keep")
            EXPORT.write_outputs(destination, outputs, check=True)
            (destination / "entries/new-widget.json").write_text("tampered")
            with self.assertRaisesRegex(ValueError, "new-widget"):
                EXPORT.write_outputs(destination, outputs, check=True)

    def test_native_composer_search_metadata_comes_from_its_source(self):
        mappings = EXPORT.gallery_sources()
        symbols = EXPORT.public_symbols(mappings["native-composer"], EXPORT.crosswalk())
        self.assertIn("NativeComposer", symbols)
        self.assertNotIn("SessionList", symbols)

    def test_source_revision_is_stable_for_website_only_changes(self):
        origin = EXPORT.provenance()
        self.assertRegex(origin["revision"], r"^[0-9a-f]{40}$")
        self.assertRegex(origin["digest"], r"^[0-9a-f]{64}$")
        self.assertEqual(origin["nativeRevision"], json.loads(
            (ROOT / "assets/tui-source.json").read_text()
        )["commit"])


if __name__ == "__main__":
    unittest.main()
