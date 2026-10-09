//! The component contract, as checks (see `CONTRIBUTING-COMPONENTS.md`).
//!
//! Not a package file: change it only with the lead.

use std::path::Path;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// Components name roles; they never name a color. A raw `Color::...`,
/// `Rgb(..)` or `Indexed(..)` in `src/components` would not follow the
/// theme, the terminal's depth or `NO_COLOR`.
#[test]
fn components_never_name_a_color() {
    let mut found = Vec::new();
    let dir = root().join("src/components");
    for entry in std::fs::read_dir(&dir).expect("src/components") {
        let path = entry.expect("entry").path();
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("component source");
        for (n, line) in source.lines().enumerate() {
            let code = line.split("//").next().unwrap_or_default();
            if ["Color::", "style::Color", "Rgb(", "Indexed("]
                .iter()
                .any(|p| code.contains(p))
            {
                found.push(format!(
                    "{}:{}: {}",
                    path.strip_prefix(root()).unwrap_or(&path).display(),
                    n + 1,
                    line.trim()
                ));
            }
        }
    }
    assert!(
        found.is_empty(),
        "name a Role and paint with theme.fg/theme.bg instead:\n{}",
        found.join("\n")
    );
}

/// The kit adds no runtime dependency without the lead.
#[test]
fn the_runtime_dependencies_are_the_current_ones() {
    let manifest = std::fs::read_to_string(root().join("Cargo.toml")).expect("Cargo.toml");
    let mut deps = Vec::new();
    let mut in_deps = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_deps = line == "[dependencies]"
                || (line.starts_with("[target") && line.ends_with(".dependencies]"));
        } else if in_deps && let Some((name, _)) = line.split_once('=') {
            deps.push(name.trim().to_string());
        }
    }
    deps.sort();
    assert_eq!(
        deps,
        [
            "crossterm",
            "libc",
            "ratatui",
            // Share underline style data with backend-free consumers.
            "ratatui-core",
            // Native character export decoding and owner context values.
            "serde",
            "serde_json",
            "unicode-segmentation",
            "unicode-width"
        ],
        "a new runtime dependency needs the lead (update this list with them)"
    );
}
