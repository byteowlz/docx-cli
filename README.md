# docx-cli

Headless Word document engine and command line tool in Rust: read and write DOCX (plus
DOC, RTF, ODT, HTML, Markdown, LaTeX, PDF export), lay out pages like Word, render pages,
compare documents into tracked-change redlines, and run editing commands without a UI.
An MCP server exposes the same commands to agents.

## Build

```bash
just build            # cargo build -p docx-cli
target/debug/docx-cli --help
```

## Common commands

```bash
docx-cli convert in.docx out.pdf              # any supported format; --markup prints tracked changes
docx-cli convert in.docx in.wcraft.json       # native JSON (see docs/headless.md)
docx-cli geometry in.docx                     # page rectangles of paragraphs, content controls, changes
docx-cli render in.docx page.png --page 2
docx-cli run --file in.docx --cmd 'review.compare={"other":"v2.docx"}' --save redline.docx
docx-cli mcp --automation-read-root DIR --automation-write-root DIR
```

`docx-cli commands` lists every command; `docs/` describes formats, layout parity and MCP.
Fonts: set `DOCX_CLI_FONT_DIR` to add a font directory (metric-compatible substitutes for
Word's fonts render closest).

## Upstream changes

`just upstream-sync` applies new commits from the upstream engine one by one, renamed by
`scripts/rebrand.pl`; `just upstream-check` lists them. Each applied commit records its
source in an `Upstream-Commit:` trailer.

## License

MIT OR Apache-2.0, at your option. See `LICENSE-MIT`, `LICENSE-APACHE` and `NOTICE`
(third-party material and original copyright).
