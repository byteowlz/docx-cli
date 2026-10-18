default:
    @just --list

build:
    cargo build -p docx-cli

check:
    cargo check -p docx-cli

# Scoped regressions for the local core changes.
test:
    cargo test -p docx-cli-doc native_json
    cargo test -p docx-cli-fonts runtime_font
    cargo test -p docx-cli-layout frames

mcp *args:
    target/debug/docx-cli mcp {{args}}

convert input output:
    target/debug/docx-cli convert "{{input}}" "{{output}}"

# Apply new upstream engine commits, renamed (see README)
upstream-sync:
    scripts/upstream-sync.sh

# List new upstream engine commits without applying them
upstream-check:
    scripts/upstream-sync.sh --check
