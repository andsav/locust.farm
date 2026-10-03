from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from check_docs import check_repository


class DocumentationChecks(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.git("init", "-q")
        for folder in ("docs", "research"):
            self.write(f"{folder}/README.md", f"# {folder}\n")

    def git(self, *arguments):
        subprocess.run(
            ["git", *arguments], cwd=self.root, check=True, capture_output=True
        )

    def write(self, path, content, tracked=True):
        destination = self.root / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(content, encoding="utf-8")
        if tracked:
            self.git("add", "--", path)

    def errors(self):
        return "\n".join(check_repository(self.root))

    def test_valid_indexes_links_and_directories(self):
        self.write("README.md", "# Project\n")
        self.write("docs/README.md", '[Design](design.md "Current design")\n')
        self.write("docs/design.md", "[Project](../README.md#project)\n")
        self.write(
            "research/README.md",
            "[Findings](<nested/experiment notes.md>)\n[Notes](nested/)\n",
        )
        self.write("research/nested/experiment notes.md", "# Findings\n")
        self.assertEqual(check_repository(self.root), [])

    def test_requires_both_root_indexes(self):
        for folder in ("docs", "research"):
            with self.subTest(folder=folder):
                self.git("rm", "-f", f"{folder}/README.md")
                self.assertIn(f"{folder}/README.md: missing tracked index", self.errors())
                self.write(f"{folder}/README.md", "# Index\n")

    def test_each_tree_requires_every_nested_document_in_its_root_index(self):
        for folder in ("docs", "research"):
            self.write(f"{folder}/nested/note.md", "# Note\n")
        errors = self.errors()
        for folder in ("docs", "research"):
            self.assertIn(
                f"{folder}/README.md: missing entry for {folder}/nested/note.md", errors
            )

    def test_duplicate_entries_normalize_paths_and_fragments(self):
        self.write("docs/note.md", "# Note\n")
        self.write(
            "docs/README.md", "[Note](note.md)\n[Again](./note.md#details)\n"
        )
        self.assertIn("duplicate entries for docs/note.md (2)", self.errors())

    def test_checks_broken_links_in_both_indexes_and_documents(self):
        for folder in ("docs", "research"):
            self.write(
                f"{folder}/README.md", "[Note](note.md)\n[Absent](absent.md)\n"
            )
            self.write(f"{folder}/note.md", "[Missing](../missing.txt)\n")
        errors = self.errors()
        for folder in ("docs", "research"):
            self.assertIn(
                f"{folder}/README.md:2: missing link target: absent.md", errors
            )
            self.assertIn(
                f"{folder}/note.md:1: missing link target: ../missing.txt", errors
            )

    def test_untracked_drafts_are_not_read_or_required_in_indexes(self):
        for folder in ("docs", "research"):
            self.write(
                f"{folder}/draft.md", "[Broken](missing.md)\n", tracked=False
            )
        self.assertEqual(check_repository(self.root), [])

    def test_linked_untracked_files_and_directories_are_rejected(self):
        self.write("docs/draft.md", "# Draft\n", tracked=False)
        self.write("docs/local/item.txt", "Local artifact", tracked=False)
        self.write("docs/README.md", "[Draft](draft.md)\n[Local](local/)\n")
        errors = self.errors()
        self.assertIn("link target is not tracked: draft.md", errors)
        self.assertIn("link target is not tracked: local/", errors)

    def test_urls_anchors_and_fenced_examples_are_ignored(self):
        self.write(
            "docs/README.md",
            "[Web](https://example.com/page)\n"
            "[Mail](mailto:person@example.com)\n"
            "[Section](#section)\n"
            "````markdown\n[Example](absent.md)\n```\n"
            "[Still an example](absent.md)\n````\n"
            "~~~markdown\n[Example](absent.md)\n~~~\n",
        )
        self.assertEqual(check_repository(self.root), [])

    def test_deleted_tracked_document_is_reported(self):
        self.write("docs/note.md", "# Note\n")
        self.write("docs/README.md", "[Note](note.md)\n")
        (self.root / "docs/note.md").unlink()
        self.assertIn("docs/note.md: tracked document is missing", self.errors())

    def test_links_cannot_leave_repository(self):
        self.write("docs/README.md", "[Outside](../../outside.md)\n")
        self.assertIn("link leaves repository: ../../outside.md", self.errors())


if __name__ == "__main__":
    unittest.main()
