# CW 5 library maintenance

Maintain the [existing Figma library](https://www.figma.com/design/4JlhR4mjtaginZwjaqZ0NU?node-id=245-2). The [repository guide](../README.md) points to design authority and the current Linear work item; [SYSTEM.md §5](../SYSTEM.md#5-mediums) owns token generation and propagation. Inspect the existing canvas before reconciling it with source.

Use the existing direct Figma connection or the native Figma app when needed;
do not open browser Figma sessions. Source generation and provenance checks
need no Figma session. See [operator decision §112](https://github.com/codewhale-hq/codewhale-ops/blob/main/CURRENT_DECISIONS.md#112-use-figma-directly-without-browser-sessions--2026-11-09).

## Inputs and generated files

| Edit | Generated — do not edit |
| --- | --- |
| [tokens.json](../tokens.json), [generate.py](../generate.py), [sync.template.js](sync.template.js) | `variables.json` and `sync.js` for variables and canonical text styles |
| [controls.spec.mjs](controls.spec.mjs), [controls.template.js](controls.template.js), [generate-controls.mjs](generate-controls.mjs), and the app component sources | `controls.manifest.json`, `controls.js` and `controls-N.js` |
| [whale/source/mark.svg](../whale/source/mark.svg), [brand.template.js](brand.template.js), [generate.py](../generate.py) | `brand.js` for the separate monochrome logo family |

`review-controls.js` and `repair-foundations.js` are maintenance operations for existing node IDs, not routine sync steps. Inspect their targets and the current task before running them.

## Existing controls

`controls.spec.mjs` describes representative content and states of the existing React controls. `generate-controls.mjs` resolves every referenced selector from the real app CSS, follows CSS Module composition, records source digests, and fails if a selector disappears. TextArea also reads its default row count from `TextArea.tsx`; the specimen uses three lines and a 74px control height. Other controls can require React behavior as well as CSS, so these are representative state specimens, not an exhaustive interaction specification.

Generate against the app checkout whose UI is being represented:

```sh
node figma/generate-controls.mjs --app /Volumes/VIXinSSD/CW/codewhale-app
node figma/generate-controls.mjs --app /Volumes/VIXinSSD/CW/codewhale-app --check
```

The generated `controls-N.js` transport batches stay below Figma's 50,000-character call limit. They and the combined payload are ignored build output. `controls.manifest.json` tracks the source inventory and digests.

Run each batch through the Figma Plugin API in the current library file. It defaults to a dry run. Prefix `const APPLY_CONTROLS = true;` to create missing families. Every existing family is skipped. `UPDATE_CONTROLS` is rejected on apply: rebuilding children would discard designer edits and instance overrides. Use reviewed exact-property patches for existing nodes instead.

The importer uses native editable nodes, CW 5 color and metric bindings, shared text styles, and the existing Button components for dialog actions. CSS weight or line-height combinations absent from the canonical nine roles use shared `CW 5/Source/…` styles with those exact source values. Existing component IDs remain untouched. Fonts, opacity, inline control sizing and cell borders are handled explicitly.

## Reviewed reconciliation

`sync.js` is dry by default; only `APPLY_FOUNDATIONS = true` enables writes. Existing valid variables and styles require no routine rewrite. `generate-controls.mjs --expected-app <sha>` rejects a checkout at another revision and records the source commit in the manifest.

`capture-screens.js` reads one known Screens page. Select one root with `CW_CAPTURE_IDS`, then use `CW_CAPTURE_OFFSET` and `CW_CAPTURE_COUNT` for bounded descendants. It returns editable hierarchy, text, geometry and instance references without mapping component properties. It changes no canvas nodes.

`patch-nodes.js` accepts a reviewed `CW_NODE_PATCHES` payload with one page ID and exact node IDs, types, `before` and `after` properties. It checks all preconditions and loads text fonts before any mutation. Allowed properties are names, component descriptions, fills, strokes and characters; geometry, child replacement and detaching instances are unavailable. Any changed precondition blocks the whole batch. Set `APPLY_PATCHES = true` only after reviewing the dry result and current canvas.

`brand.js` is dry by default. The proposed application renames blue character master `269:8` without changing its geometry or instances, and imports the original monochrome SVG paths into a separate `Codewhale/Logo monochrome` family. Existing same-name families are preserved. Pending standalone variants block retries until their IDs are inspected; do not delete them blindly. The older `231:216` symbol is outside this batch until its current page and content are inspected. Apply only with `APPLY_BRAND = true` after review. Capture the returned IDs and inspect both themes before recording the family as current in the node inventory.

These scripts are direct-tool bodies with top-level await/return. Each navigates at most one page. Generation and dry-run plans do not establish live canvas parity; screenshots remain references, not editable deliverables. Platform consumers retain their own current token version until separately authorized.

`review-controls.js` creates light and dark instance sheets. `repair-foundations.js` updates the existing metrics board's version, makes the scrim visible over contrasting grounds and repairs the clipped focus specimen. Both target the existing library's known node IDs and avoid unrelated work.

## Status and historical evidence

[CW-7930](https://linear.app/shannonlabs/issue/CW-7930) owns current status and
links to the existing library work. Dated screenshots and readback receipts
are under `CW/artifacts/design-takeover-20261109/`; use the workspace artifact
index for the latest evidence rather than copying counts into more READMEs.

The control specimens establish representative Figma coverage and source
metrics. Runtime keyboard, accessibility, provider and installed-app
acceptance require their own evidence. Figma Archive pages preserve prior
work; they are not instructions to recreate it.
