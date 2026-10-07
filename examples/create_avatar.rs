//! Create a Native avatar plugin from a PNG sheet. No install or approval changes.
use codewhale_ratatui::avatar::{ACTS, Action, Pack, slug};
use std::{collections::BTreeMap, io, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut values = BTreeMap::new();
    while let Some(key) = args.next() {
        if key == "--help" {
            println!(
                "create_avatar --sheet art.png --out my-avatar --id my-avatar --name 'My avatar' [--columns 1] [--rows 1] [--actions rest,done] [--duration 200]\nEach row is a named action; each column is a frame. Other states fall back to rest. Existing output directories are never overwritten."
            );
            return Ok(());
        }
        if ![
            "--sheet",
            "--out",
            "--id",
            "--name",
            "--columns",
            "--rows",
            "--actions",
            "--duration",
        ]
        .contains(&key.as_str())
        {
            return Err(format!("Unknown option {key}; see --help").into());
        }
        let value = args.next().ok_or("Option needs a value; see --help")?;
        if values.insert(key, value).is_some() {
            return Err("Repeated option".into());
        }
    }
    let required = |key: &str| {
        values
            .get(key)
            .ok_or_else(|| format!("{key} is required; see --help"))
    };
    let id = required("--id")?;
    if !slug(id)
        || id.contains('_')
        || id.contains("--")
        || !id.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
        || !id.as_bytes().last().is_some_and(u8::is_ascii_alphanumeric)
    {
        return Err(
            "Use a lowercase avatar id with letters, numbers and internal single hyphens".into(),
        );
    }
    let name = required("--name")?;
    let sheet = PathBuf::from(required("--sheet")?);
    let out = PathBuf::from(required("--out")?);
    let number = |key: &str, default: u16| -> Result<u16, Box<dyn std::error::Error>> {
        Ok(values
            .get(key)
            .map(|s| s.parse())
            .transpose()?
            .unwrap_or(default))
    };
    let columns = number("--columns", 1)?;
    let rows = number("--rows", 1)?;
    let duration = number("--duration", 200)?;
    if columns == 0 || rows == 0 || u32::from(columns) * u32::from(rows) > 128 {
        return Err("The sheet must have 1–128 cells".into());
    }
    let names: Vec<_> = values
        .get("--actions")
        .map_or("rest", String::as_str)
        .split(',')
        .collect();
    if names.len() != usize::from(rows) || !names.contains(&"rest") {
        return Err("Name each row in --actions and include rest (for example --rows 2 --actions rest,done)".into());
    }
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(&sheet)?
        .take(codewhale_ratatui::avatar::MAX_PNG_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() < 33
        || bytes.len() > codewhale_ratatui::avatar::MAX_PNG_BYTES
        || &bytes[..8] != b"\x89PNG\r\n\x1a\n"
    {
        return Err("Use a PNG sheet of at most 4 MiB".into());
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into()?);
    let height = u32::from_be_bytes(bytes[20..24].try_into()?);
    if width == 0
        || height == 0
        || width > 2048
        || height > 2048
        || width % u32::from(columns) != 0
        || height % u32::from(rows) != 0
    {
        return Err(
            "Sheet dimensions must be at most 2048px and divide evenly into the grid".into(),
        );
    }
    let mut actions = BTreeMap::new();
    for (row, action) in names.iter().enumerate() {
        let first = row as u16 * columns;
        if actions
            .insert(
                (*action).into(),
                Action {
                    frames: (first..first + columns).collect(),
                    durations_ms: vec![duration; usize::from(columns)],
                    poster: first,
                    repeat: !matches!(*action, "done" | "greet"),
                    motion: Default::default(),
                },
            )
            .is_some()
        {
            return Err("Action names must be unique".into());
        }
    }
    let states = ACTS
        .iter()
        .map(|act| {
            (
                (*act).into(),
                if actions.contains_key(*act) {
                    *act
                } else {
                    "rest"
                }
                .into(),
            )
        })
        .collect();
    let mut views = BTreeMap::from([("front".into(), "rest".into())]);
    for view in ["side", "back"] {
        if actions.contains_key(view) {
            views.insert(view.into(), view.into());
        }
    }
    let pack = Pack {
        version: 1,
        id: id.clone(),
        name: name.clone(),
        atlases: vec!["sheet.png".into()],
        columns,
        rows,
        tile_width: (width / u32::from(columns)) as u16,
        tile_height: (height / u32::from(rows)) as u16,
        actions,
        states,
        views,
        compact: None,
    };
    pack.validate_png(&bytes).map_err(io::Error::other)?;
    image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)?;
    // Validate everything before creating a new directory; never replace an existing plugin.
    std::fs::create_dir(&out)?;
    std::fs::create_dir(out.join("avatars"))?;
    std::fs::write(out.join("avatars/sheet.png"), bytes)?;
    std::fs::write(
        out.join("avatars/avatar.json"),
        serde_json::to_string_pretty(&pack)? + "\n",
    )?;
    let manifest = serde_json::json!({"name":id,"version":"0.1.0","description":format!("{name} avatar"),
        "extensions":{"net.codewhale":{"native":{"path":"index.mjs"}}}});
    std::fs::write(
        out.join("plugin.json"),
        serde_json::to_string_pretty(&manifest)? + "\n",
    )?;
    std::fs::write(
        out.join("index.mjs"),
        "export const inject = ['avatars'];\nexport function apply(ctx) {\n  ctx.avatars.registerPack({ path: 'avatars/avatar.json' });\n}\n",
    )?;
    std::fs::write(
        out.join("README.md"),
        format!(
            "# {name}\n\nPreview without installing:\n\n```sh\ncargo run --example avatar -- --pack {}/avatars/avatar.json\n```\n\nInstall this folder with Codewhale's normal Native plugin review and enable flow. Select {name} under Character in the whale habitat, or use /pet avatar in the terminal.\n\nEach sheet row is an action. Edit avatars/avatar.json to bind states, add named views, tune timing, or set a compact face crop. Unmapped states use rest. Custom action previews never change real work status. Add other behavior with the existing commands/tools/events services in the same plugin. Native review must be repeated after changing reviewed files.\n",
            out.display()
        ),
    )?;
    println!(
        "Created {}. Preview its avatars/avatar.json, then use the normal Native plugin review flow.",
        out.display()
    );
    Ok(())
}
