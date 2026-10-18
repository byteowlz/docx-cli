#!/usr/bin/env bash
# Apply scripts/rebrand.pl to every tracked text file and rename paths. Idempotent.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
keep='^(LICENSE-MIT|LICENSE-APACHE|NOTICE|ATTRIBUTION\.md|contributors/|scripts/rebrand\.pl|Cargo\.lock)'
git grep -I -l -i -E 'wordcraft' -- . | grep -v -E "$keep" | while IFS= read -r f; do
  perl scripts/rebrand.pl "$f" > "$f.rebrand" && mv "$f.rebrand" "$f"
done
git ls-files | grep -i wordcraft | grep -v -E "$keep" | while IFS= read -r f; do
  new=$(printf '%s' "$f" | perl scripts/rebrand.pl)
  mkdir -p "$(dirname "$new")"; git mv "$f" "$new"
done
