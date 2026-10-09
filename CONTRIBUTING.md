# Contributing

Codewhale Ratatui provides reusable terminal components and native Codewhale
views. Start with the [getting-started guide](GETTING-STARTED.md) and
[component catalogue](COMPONENTS.md) to find the existing component or example
closest to your change.

## Set up

Use Rust 1.89 or newer, as declared in `Cargo.toml`.
`rust-toolchain.toml` selects the rolling stable channel, not a fixed version;
CI checks the declared minimum separately.

```sh
cargo build --locked
cargo run --locked --example starter
cargo run --locked --example gallery
```

In the gallery, `p` changes the terminal profile and `w` changes the width.

## Make and verify a change

- Follow [Adding a component](CONTRIBUTING-COMPONENTS.md) for the rendering,
  text, input, accessibility, gallery and snapshot contracts. It also explains
  file ownership when several contributors are working together.
- Preserve the native presentation described in [DESIGN.md](DESIGN.md).
  Check relevant states at narrow widths and in the no-color, ASCII and
  ANSI-16 profiles; color must not be the only way to recognize a state.
- Use the [README's source-asset instructions](README.md#update-source-palettes-and-design-assets)
  when updating generated palettes, tokens or whale data. Review generated
  output with its source change.
- Run the focused tests for the paths you changed, and inspect intentional
  snapshot changes before accepting them. The component guide explains the
  snapshot workflow.

Before opening a pull request, check formatting and lint the targets:

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
```

For example, component contract changes can be checked with
`cargo test --locked --test contract`. CI runs the full test suite across
supported platforms and the minimum Rust version, plus documentation,
consumer/package and generated-preview checks. [QUALITY.md](QUALITY.md)
explains those gates and how to run the relevant ones locally.

## Pull requests

Keep each PR focused. Describe the problem, resulting behavior and checks you
actually ran; include before/after previews for visual changes. Use a short
commit prefix such as `feat:`, `fix:`, `docs:` or `chore:`. Stage only the files
belonging to your change, especially in a shared checkout.

Report vulnerabilities privately as described in [SECURITY.md](SECURITY.md).
Contributions are covered by the repository's [MIT license](LICENSE).
