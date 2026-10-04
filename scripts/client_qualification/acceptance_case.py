"""Deployment-specific late requirement for collaboration acceptance.

The public artifact and objective are the pilot's archive helper. Only the
researcher knows the service's reserved root. This allows a correct portable
initial implementation to require revision without prescribing bad code.
"""
from pathlib import Path

from . import collaboration_case as pilot

RESERVED_ROOT_NOTE = """
The deployment owns a root entry named `.archive-index` for extraction metadata.
Archive members must not occupy that root entry or anything below it. Treat the
reserved root name case-insensitively for compatibility with existing clients.
This restriction applies only to the first normalized path component: a nested
entry such as `assets/.archive-index` and the distinct root `.archive-index-v2`
remain valid. Apply the restriction after normalizing separators.
"""

ORACLE = pilot.ORACLE.replace('cases = [', '''cases = [
    (".archive-index", None, "deployment metadata root rejected"),
    (".ARCHIVE-INDEX/data.json", None, "reserved root compared case-insensitively"),
    (".archive-index\\\\data.json", None, "reserved root checked after separator normalization"),
    ("assets/.archive-index", "assets/.archive-index", "nested metadata-like name remains valid"),
    (".archive-index-v2", ".archive-index-v2", "distinct root name remains valid"),''')


def seed(research_workspace, builder_workspace):
    fixture = pilot.seed(research_workspace, builder_workspace)
    path = Path(fixture['researcher_only']['private_fixture'])
    path.write_text(path.read_text() + RESERVED_ROOT_NOTE)
    public = Path(builder_workspace) / 'README.md'
    public.write_text(public.read_text() + '\nRuntime: Python 3.12 or newer.\n')
    fixture['researcher_only']['deployment_rule'] = RESERVED_ROOT_NOTE
    return fixture


def verify(workspace):
    return pilot.verify(workspace, oracle=ORACLE, case_count=14)
