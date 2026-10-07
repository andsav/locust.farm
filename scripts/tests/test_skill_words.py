"""Keep installed agent instructions on the same vocabulary as the guide."""

from pathlib import Path
import re
import unittest


class SkillWordsTests(unittest.TestCase):
    def test_the_skill_uses_the_words_people_read(self):
        skill = (Path(__file__).resolve().parents[2] / "skills/locust/SKILL.md").read_text()
        self.assertNotRegex(
            skill,
            re.compile(r"\b(?:grants?|authoriz\w*|administrators?|participants?|principals?|viewers?)\b", re.I),
        )


if __name__ == "__main__":
    unittest.main()
