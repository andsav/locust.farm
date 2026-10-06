#!/usr/bin/env python3
"""Check tracked docs/research indexes and local inline Markdown links.

Supports [label](path), optional quoted titles, and <angle-bracket paths>.
Fenced code, URLs, and anchor-only links are ignored; fragments are not checked.
Reference-style links and nested/escaped Markdown syntax are outside this check.
Directories are valid link targets only when they contain tracked files.
Source links in the explicitly pinned historical plans below may refer to
files since removed; those targets must be blobs in that plan's source commit.
Current guides and links to documentation always require current targets.
"""

from collections import Counter
from pathlib import Path
import re
import subprocess
import sys
from urllib.parse import unquote, urlsplit


MARKDOWN_SUFFIXES = {".md", ".markdown"}
FENCE = re.compile(r"^ {0,3}(`{3,}|~{3,})(.*)$")
LINK = re.compile(
    r"\[[^\]\n]*\]\(\s*(?:<([^>\n]+)>|([^\s()]+))"
    r"(?:\s+(?:\"[^\"\n]*\"|'[^'\n]*'))?\s*\)"
)

# These plans describe the source snapshots named in their introductions and
# are maintained separately from implementation. Removing superseded source
# must not require rewriting a historical plan or retaining a dead module.
SOURCE_BASELINES = {
    "docs/roles-and-permissions-plan.md": "cfb5b450312efa402fc3b3931907fa81bc207d6b",
    "docs/roles-and-permissions-plan-details.md": "cfb5b450312efa402fc3b3931907fa81bc207d6b",
    "docs/host-safety-and-ending-plan.md": "48bb12c3aa45dcd6d70a521a95ca77c58ea1f776",
    "docs/joinable-farms-plan.md": "0685f8042feb17a45c6d9cea587eafe91376d05f",
}


def inline_links(contents: str):
    """Yield (line number, destination) outside backtick and tilde fences."""
    fence_character = ""
    fence_length = 0
    for line_number, line in enumerate(contents.splitlines(), 1):
        fence = FENCE.match(line)
        if fence:
            marker, rest = fence.groups()
            if not fence_character:
                fence_character, fence_length = marker[0], len(marker)
            elif (
                marker[0] == fence_character
                and len(marker) >= fence_length
                and not rest.strip()
            ):
                fence_character = ""
            continue
        if not fence_character:
            for match in LINK.finditer(line):
                yield line_number, match.group(1) or match.group(2)


def check_repository(root: Path) -> list[str]:
    root = root.resolve()
    tracked = set(
        subprocess.check_output(
            ["git", "ls-files", "-z"], cwd=root
        ).decode().split("\0")
    ) - {""}
    documents = {
        path
        for path in tracked
        if path.split("/", 1)[0] in {"docs", "research"}
        and Path(path).suffix.lower() in MARKDOWN_SUFFIXES
    }
    errors = []
    indexed = {"docs": Counter(), "research": Counter()}
    historical_sources = {}

    def historical_source(document: str, relative: str) -> bool:
        baseline = SOURCE_BASELINES.get(document)
        if not baseline or not relative.startswith("crates/"):
            return False
        key = (baseline, relative)
        if key not in historical_sources:
            found = subprocess.run(
                ["git", "cat-file", "-t", f"{baseline}:{relative}"],
                cwd=root, capture_output=True, text=True,
            )
            historical_sources[key] = found.returncode == 0 and found.stdout.strip() == "blob"
        return historical_sources[key]

    for document in sorted(documents):
        source = root / document
        if not source.is_file():
            errors.append(f"{document}: tracked document is missing")
            continue
        for line, destination in inline_links(source.read_text(encoding="utf-8")):
            location = f"{document}:{line}"
            try:
                url = urlsplit(destination)
            except ValueError:
                errors.append(f"{location}: invalid link: {destination}")
                continue
            if url.scheme or url.netloc or not url.path:
                continue
            target = (source.parent / unquote(url.path)).resolve()
            try:
                relative = target.relative_to(root).as_posix()
            except ValueError:
                errors.append(f"{location}: link leaves repository: {destination}")
                continue
            if not target.exists():
                if historical_source(document, relative):
                    continue
                errors.append(f"{location}: missing link target: {destination}")
                continue
            if target.is_dir():
                prefix = "" if relative == "." else relative + "/"
                is_tracked = any(path.startswith(prefix) for path in tracked)
            else:
                is_tracked = relative in tracked
            if not is_tracked:
                errors.append(f"{location}: link target is not tracked: {destination}")
                continue
            folder = document.split("/", 1)[0]
            if (
                document == f"{folder}/README.md"
                and relative in documents
                and relative.startswith(folder + "/")
                and relative != document
            ):
                indexed[folder][relative] += 1

    for folder, entries in indexed.items():
        index = f"{folder}/README.md"
        if index not in documents:
            errors.append(f"{index}: missing tracked index")
        expected = {path for path in documents if path.startswith(folder + "/")}
        for document in sorted(expected - {index}):
            count = entries[document]
            if count == 0:
                errors.append(f"{index}: missing entry for {document}")
            elif count > 1:
                errors.append(f"{index}: duplicate entries for {document} ({count})")
    return errors


def main() -> int:
    try:
        errors = check_repository(Path(__file__).resolve().parents[1])
    except (OSError, UnicodeError, subprocess.CalledProcessError) as error:
        print(f"Documentation check failed: {error}", file=sys.stderr)
        return 1
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Documentation checks passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
