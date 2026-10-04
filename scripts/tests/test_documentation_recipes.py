"""Keep executable documentation fences explicit and unambiguous."""
import sys
from pathlib import Path
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from check_documentation import recipes


class RecipeTests(unittest.TestCase):
    def test_only_marked_closed_bash_blocks_execute(self):
        block = '# locust-doc-test: example\nprintf done\n'
        self.assertEqual(recipes('```bash\n' + block + '```\n\n```sh\necho skip\n```\n'), [('example', block)])
        self.assertEqual(recipes('```bash\necho skip\n```\n'), [])

    def test_malformed_missing_empty_and_duplicate_fences_fail(self):
        cases = [
            '# locust-doc-test: outside\n',
            '```bash\n# locust-doc-test: missing\necho x\n',
            '```bash\n# locust-doc-test: bad name\necho x\n```\n',
            '```bash\n# locust-doc-test: empty\n```\n',
            '```bash\n# locust-doc-test: duplicate\necho x\n```\n' * 2,
        ]
        for source in cases:
            with self.subTest(source=source), self.assertRaises(ValueError):
                recipes(source)


if __name__ == '__main__':
    unittest.main()
