#!/usr/bin/env python3
"""Fail closed if a packaged Markdown asset links to a non-packaged target.

Given the release gate's package-asset list as arguments, this checks that
every relative Markdown link inside a packaged `.md` file resolves to another
packaged file. Absolute (`http(s)://`, `mailto:`) links and bare fragments are
ignored. A packaged crate archive must not carry links that 404 for a
crates.io or docs.rs reader; links to governance or repository-infrastructure
files that are intentionally not packaged must be absolute URLs.
"""

import os
import re
import sys

LINK = re.compile(r"\[[^\]]*\]\(([^)]+)\)")


def main(argv: list[str]) -> int:
    assets = {os.path.normpath(a) for a in argv[1:]}
    markdown = sorted(a for a in assets if a.endswith(".md"))
    dangling: list[tuple[str, str, str]] = []
    for path in markdown:
        base = os.path.dirname(path)
        try:
            text = open(path, encoding="utf-8").read()
        except OSError as error:
            dangling.append((path, f"<unreadable: {error}>", ""))
            continue
        for raw in LINK.findall(text):
            if raw.startswith(("http://", "https://", "mailto:", "#")):
                continue
            target = raw.split("#", 1)[0]
            if not target:
                continue
            resolved = os.path.normpath(os.path.join(base, target))
            if resolved not in assets:
                dangling.append((path, raw, resolved))
    if dangling:
        print(
            "release gate: packaged docs link to non-packaged targets "
            "(package them or make the link an absolute URL):",
            file=sys.stderr,
        )
        for path, raw, resolved in dangling:
            print(f"  {path}: [{raw}] -> {resolved}", file=sys.stderr)
        return 1
    print(
        f"package doc links: {len(markdown)} packaged Markdown files, "
        "all relative links resolve within the package"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
