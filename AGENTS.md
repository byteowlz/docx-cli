# AGENTS.md

docx-cli: headless DOCX engine and CLI (Rust workspace). `apps/docx-cli` is the binary;
`crates/` hold the document model (`doc`), DOCX I/O (`docx`), layout, PDF/PNG output,
proofing, MCP and the command engine (`engine`).

## Workflow

- After changes: `rustfmt` only the files you touched (never `cargo fmt --all`: renamed crate paths would re-wrap
  hundreds of upstream lines and break upstream patches), then `cargo test -p <touched crates>`; `cargo check --workspace --all-targets` before committing.
- Tests assert on full values. Every bug fix lands with a regression test.
- Known environment failures on macOS, also failing upstream: `render::tests::legacy_font_effects_are_drawn`;
  `engine::tests::italian_proofing_through_the_review_commands` only fails when run in parallel.

## Engineering rules (kept from the upstream engine)

- Never crash: no `unwrap`/`expect`/`panic!`/`unreachable!`/`todo!` in non-test code, no `unsafe`;
  errors are `Result` through the crate's error type. Input-derived numbers are hostile: `get()`, char
  boundaries, checked arithmetic, capped allocations. `engine::tests::hostile_params_never_panic` stays green.
- Assets: no Microsoft (or other vendor) icons, images, templates or fonts. Every asset file has a row in
  `ATTRIBUTION.md`. Never bundle proprietary fonts; use metric-compatible substitutes.
- Clean room: formats come from public specs (ECMA-376/ISO 29500, RTF spec, ODF 1.3). Never copy
  GPL/LGPL/AGPL code (LibreOffice, AbiWord, pandoc).

## Upstream

- `upstream` remote is the original engine. Take its improvements with `just upstream-sync`
  (format-patch, rename through `scripts/rebrand.pl`, `git am -3`, `Upstream-Commit:` trailer).
- Keep local changes small and focused so upstream patches keep applying. Do not reformat or
  rename beyond `scripts/rebrand.pl`; new names go into that script so patches follow.
- Never edit `LICENSE-*`, `NOTICE`, `ATTRIBUTION.md` or `contributors/` beyond adding rows; they carry
  the original copyright and are required by the license.
- Never push to new remotes or publish crates without the owner's approval.
