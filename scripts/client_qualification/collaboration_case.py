"""Small hidden-constraint case for real-model Locust collaboration.

The researcher receives the operational constraint in a private workspace.
The builder receives only the public starter project and must learn that
constraint through the shared Locust finding.  The oracle below is harness
owned and is never written into the builder workspace.
"""

import hashlib
import os
from pathlib import Path
import subprocess
import sys


PRIVATE_FIXTURE = """Archive members are produced by clients on both Windows and POSIX hosts, then extracted by our Linux service. The extractor must treat a backslash in a member name as a path separator even when it runs on POSIX. Normalize safe names to forward slashes. Reject names that become absolute or contain a parent-directory component after that normalization. Ordinary nested relative names remain valid.

Observed examples from the archive pipeline:
  assets\\logo.svg -> assets/logo.svg
  assets\\..\\outside.txt -> reject
  C:\\temp\\outside.txt -> reject
  \\\\server\\share\\outside.txt -> reject
"""


PUBLIC_README = """# Archive member path helper

Implement `normalize_member_path(name: str) -> str | None` in `safe_member.py`.

The archive service needs a portable, relative destination name for each
untrusted archive member. Return a normalized POSIX-style relative name when
it is safe, or `None` when it must not be extracted. Keep the helper small and
use only the Python standard library. The current implementation is a starter
and may not satisfy the service's full requirements.
"""


STARTER = '''"""Normalize an untrusted archive member to a relative path."""

from pathlib import PurePosixPath


def normalize_member_path(name: str) -> str | None:
    if not isinstance(name, str) or not name or "\\x00" in name:
        return None
    path = PurePosixPath(name)
    if path.is_absolute() or ".." in path.parts:
        return None
    return path.as_posix()
'''


RESEARCHER_PROMPT = """Investigate the private operational note in your assigned workspace. Identify the archive-path behavior that could let an entry escape its intended extraction directory on the service platform. Publish a concise, attributed finding to the Locust goal so the implementer can act on it. Do not modify the implementation workspace."""


BUILDER_PROMPT = """Implement the archive member path helper described by the project brief in your assigned workspace. Inspect the project and relevant shared Locust context, use the useful finding you discover, and publish the completed change as a Locust contribution. The result should be suitable for an archive service handling untrusted member names."""


ORACLE = r'''import importlib.util
import sys

path = sys.argv[1]
spec = importlib.util.spec_from_file_location("locust_case_safe_member", path)
if spec is None or spec.loader is None:
    raise RuntimeError("could not load safe_member.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
normalize = module.normalize_member_path
cases = [
    ("images/logo.svg", "images/logo.svg", "ordinary nested POSIX name"),
    ("images\\logo.svg", "images/logo.svg", "Windows separator normalized on POSIX"),
    ("images\\..\\outside.txt", None, "Windows parent traversal rejected"),
    ("C:\\temp\\outside.txt", None, "Windows drive path rejected"),
    ("\\\\server\\share\\outside.txt", None, "Windows network-root path rejected"),
    ("/etc/passwd", None, "POSIX absolute path rejected"),
    ("images/../outside.txt", None, "POSIX parent traversal rejected"),
    ("", None, "empty member rejected"),
    ("images/\x00logo.svg", None, "NUL-containing member rejected"),
]
for name, expected, label in cases:
    actual = normalize(name)
    if actual != expected:
        raise AssertionError(f"{label}: expected {expected!r}, got {actual!r}")
print("passed " + str(len(cases)) + " archive path cases")
'''


def _write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def seed(research_workspace: Path, builder_workspace: Path) -> dict:
    """Create disjoint role workspaces and return natural-language prompts.

    The private summary and fixture path are nested under `researcher_only`;
    callers should construct the builder run from `prompts["builder"]` alone.
    """
    research_workspace = Path(research_workspace).resolve()
    builder_workspace = Path(builder_workspace).resolve()
    if (research_workspace == builder_workspace or
            research_workspace in builder_workspace.parents or
            builder_workspace in research_workspace.parents):
        raise ValueError("researcher and builder workspaces must be disjoint")

    _write(research_workspace / "private" / "archive-operations.md", PRIVATE_FIXTURE)
    _write(builder_workspace / "README.md", PUBLIC_README)
    _write(builder_workspace / "safe_member.py", STARTER)

    return {
        "prompts": {
            "researcher": RESEARCHER_PROMPT,
            "builder": BUILDER_PROMPT,
        },
        "researcher_only": {
            "rule_summary": (
                "Normalize backslashes as separators before checking archive member "
                "paths for absolute roots and parent traversal; return safe names "
                "with POSIX separators."
            ),
            "private_fixture": str(research_workspace / "private" / "archive-operations.md"),
        },
        "builder_files": ["README.md", "safe_member.py"],
    }


def verify(builder_workspace: Path, *, oracle=ORACLE, case_count=9) -> dict:
    """Run the hidden, independent behavior oracle against the builder artifact."""
    builder_workspace = Path(builder_workspace).resolve()
    artifact = builder_workspace / "safe_member.py"
    assertions = []
    if not artifact.is_file() or artifact.is_symlink():
        return {
            "passed": False,
            "artifact": str(artifact),
            "assertions": [{"name": "artifact_present", "passed": False}],
            "reason": "safe_member.py is missing or is not a regular file",
        }

    assertions.append({"name": "artifact_present", "passed": True})
    env = {"PATH": os.defpath, "PYTHONIOENCODING": "utf-8", "PYTHONDONTWRITEBYTECODE": "1"}
    try:
        result = subprocess.run(
            [sys.executable, "-I", "-c", oracle, str(artifact)],
            cwd=builder_workspace,
            env=env,
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError as error:
        assertions.append({"name": "behavior_oracle", "passed": False})
        return {
            "passed": False,
            "artifact": str(artifact),
            "assertions": assertions,
            "reason": type(error).__name__,
        }

    marker = "passed " + str(case_count) + " archive path cases"
    passed = result.returncode == 0 and marker in result.stdout
    assertions.append({
        "name": "behavior_oracle",
        "passed": passed,
        "case_count": case_count,
        "success_marker_observed": marker in result.stdout,
        "stdout_bytes": len(result.stdout.encode("utf-8")),
        "stderr_bytes": len(result.stderr.encode("utf-8")),
        "stdout_sha256": hashlib.sha256(result.stdout.encode("utf-8")).hexdigest(),
        "stderr_sha256": hashlib.sha256(result.stderr.encode("utf-8")).hexdigest(),
        "stdout": result.stdout,
        "stderr": result.stderr,
        "exit_code": result.returncode,
    })
    return {
        "passed": all(item["passed"] for item in assertions),
        "artifact": str(artifact),
        "assertions": assertions,
    }
