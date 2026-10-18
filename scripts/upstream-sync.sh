#!/usr/bin/env bash
# Apply new upstream commits one by one, renamed through scripts/rebrand.pl.
#   scripts/upstream-sync.sh          apply everything new
#   scripts/upstream-sync.sh --check  list new commits, apply nothing
# Each applied commit keeps its author and gets an "Upstream-Commit:" trailer; the newest
# trailer (or .upstream-base) is where the next sync starts. On a conflict git am stops:
# resolve, `git am --continue`, then add the trailer with
# `git commit --amend --no-edit --trailer "Upstream-Commit: <sha>"` and run this again.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
[ -z "$(git status --porcelain)" ] || { echo "working tree not clean" >&2; exit 1; }
git fetch -q upstream
base=$(git log --format='%(trailers:key=Upstream-Commit,valueonly)' | grep -m1 . || cat .upstream-base)
base=$(echo "$base" | tr -d '[:space:]')
# Paths removed here (desktop/web apps, packaging, brand) and files that are ours or credits.
skip=(':(exclude)apps/wordcraft' ':(exclude)apps/wordcraft-web' ':(exclude)crates/ui-egui' ':(exclude)packaging'
      ':(exclude).github' ':(exclude)docs/brand' ':(exclude)assets/app-icon' ':(exclude)README.md' ':(exclude)AGENTS.md'
      ':(exclude)CLAUDE.md' ':(exclude)NOTICE' ':(exclude)ATTRIBUTION.md' ':(exclude)LICENSE-MIT' ':(exclude)LICENSE-APACHE'
      ':(exclude)contributors' ':(exclude)Cargo.lock' ':(exclude)ROADMAP.md' ':(exclude)docs/images' ':(exclude)docs/ui-parity.md'
      ':(exclude)docs/releasing.md' ':(exclude)docs/target-app-parity.md' ':(exclude)docs/roadmap.md' ':(exclude)docs/contributors.md'
      ':(exclude)docs/localization-*.md')
commits=$(git rev-list --reverse --first-parent --no-merges "$base"..upstream/main)
[ -n "$commits" ] || { echo "up to date with upstream ($base)"; exit 0; }
echo "$(echo "$commits" | wc -l | tr -d ' ') new upstream commits since ${base:0:9}"
if [ "${1:-}" = --check ]; then git log --oneline --no-walk $commits; exit 0; fi
patch=$(mktemp)
for c in $commits; do
  git format-patch -1 --stdout "$c" -- . "${skip[@]}" | perl scripts/rebrand.pl > "$patch"
  credits=$(git diff --name-only "$c^" "$c" -- NOTICE ATTRIBUTION.md LICENSE-MIT LICENSE-APACHE contributors | tr '\n' ' ')
  [ -z "$credits" ] || echo "note: ${c:0:9} changes credit files ($credits); review them by hand"
  if ! grep -q '^diff --git' "$patch"; then
    # Nothing left for this repository; record it so the next run moves on.
    git commit -q --allow-empty -m "Upstream ${c:0:9}: nothing applicable" --trailer "Upstream-Commit: $c"; continue
  fi
  git am -3 -q "$patch" || { echo "conflict applying upstream ${c:0:9}; see the note at the top of $0" >&2; exit 1; }
  git commit -q --amend --no-edit --trailer "Upstream-Commit: $c"
  echo "applied ${c:0:9} $(git log -1 --format=%s)"
done
rm -f "$patch"
cargo check -q -p docx-cli
