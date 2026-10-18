# Headless native JSON exchange

docx-cli is headless: `cargo build` / `just build` builds the CLI with its core
and MCP crates. Run `just mcp` to serve MCP over stdio; pass explicit read/write
roots as documented in [mcp.md](mcp.md). tmpltr's DOCX adapter uses the same engine.

docx-cli's `.json` / `.wcraft.json` format is the native `Document` object, not a
versioned envelope. Use the CLI:

```bash
docx-cli convert scaffold.docx scaffold.wcraft.json
docx-cli convert scaffold.wcraft.json output.docx
docx-cli convert output.docx reopened.wcraft.json
```

This is also an interchange path for optional tools such as tmpltr that import
or export DOCX; it does not require a tmpltr dependency in docx-cli.

## Embedded payloads

- `media` maps image keys to byte arrays (`0`–`255`). Image objects reference those
  keys, so keep both the reference and its bytes when editing a scaffold/logo.
- `passthrough` maps source-part names to opaque byte arrays retained by the
  importer, including supported unknown OOXML parts and preservation metadata.
  Keep these entries intact; they are not all standalone package parts.
- Both maps serialize in deterministic key order. Empty maps are omitted; absent
  fields in older JSON default to empty maps. Empty payload entries are retained.
  Old JSON that already discarded bytes cannot recover them.

**Privacy:** JSON exported from an imported document carries private original
source data, not just visible text. Media, passthrough parts, document metadata
(`core`, `customProps`) and settings such as `attached_template` may contain
sensitive information. Review before sharing; use synthetic/public scaffolds in
examples and never publish private source payloads.

## Limits and verification

Native JSON read/write preserves these maps byte-for-byte. Synthetic doc tests
cover full `Document` equality, missing/empty maps, deterministic output and
invalid bytes: `cargo test -p docx-cli-doc native_json`. An external engine probe
also verified two JSON → DOCX → JSON cycles with an original PNG logo and
synthetic custom XML, preserving image references, alt text and payload bytes.

This is **not an arbitrary-format losslessness guarantee**: DOCX import/export
may normalize supported structure or omit unsupported features, and other
formats have their own fidelity limits. Inspect exported results. JSON byte
arrays are larger than binary payloads; callers should bound untrusted input
sizes. The native JSON engine reader deserializes directly and repairs empty
stories; it does not apply the DOCX package reader's decompression limits.

## Runtime font directories

Set `DOCX_CLI_FONT_DIR` before launching a native CLI/app to scan extra font
folders recursively alongside normal system/user fonts, without installing fonts
or changing `HOME`. It accepts a platform path list: `:` on macOS/Linux, `;` on
Windows. Empty entries are ignored. Paths are literal (no `~` expansion).

```bash
DOCX_CLI_FONT_DIR="$PWD/demo/fonts:$PWD/extra-fonts" docx-cli convert scaffold.wcraft.json output.pdf
```

The setting is read when the process-wide font database is first initialized;
restart the process to change it. It is runtime-only, separate from the optional
build-time `CRAFT_FONTS_DIR` input, and has no effect on WASM.
