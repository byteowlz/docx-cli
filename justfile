# Headless byteowlz build: retains CLI + MCP, does not build graphical apps.
default:
    @just --list

build:
    cargo build -p wordcraft-cli

check:
    cargo check -p wordcraft-cli

# Scoped regressions for the local core changes.
test:
    cargo test -p wordcraft-doc native_json
    cargo test -p wordcraft-fonts runtime_font
    cargo test -p wordcraft-layout frames

mcp *args:
    target/debug/wordcraft-cli mcp {{args}}

convert input output:
    target/debug/wordcraft-cli convert "{{input}}" "{{output}}"
