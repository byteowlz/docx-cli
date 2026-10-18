#!/usr/bin/env perl
# Upstream names to docx-cli names. Used once on the tree and on every upstream patch
# (just upstream-sync), so upstream changes apply to the renamed code.
# Credits stay untouched: LICENSE-*, NOTICE, ATTRIBUTION.md and contributors/ are never rewritten.
use strict; use warnings;
while (my $l = <>) {
    $l =~ s{https://getartcraft\.com/apps/wordcraft}{https://github.com/byteowlz/docx-cli}g;
    $l =~ s{https://github\.com/storytold/wordcraft}{https://github.com/byteowlz/docx-cli}g;
    $l =~ s/wordcraft-cli/docx-cli/g;
    $l =~ s/wordcraft_cli/docx_cli/g;
    $l =~ s/WORDCRAFT_/DOCX_CLI_/g;
    $l =~ s/\bWORDCRAFT\b/DOCX_CLI/g;
    $l =~ s/wordcraft-/docx-cli-/g;
    $l =~ s/wordcraft_/docx_cli_/g;
    $l =~ s/Word[Cc]raft/docx-cli/g;
    $l =~ s/\bwordcraft\b/docx-cli/g;
    print $l;
}
