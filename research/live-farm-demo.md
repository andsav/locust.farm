# Live farm rehearsal — October 4, 2026

Status: completed; final artifact accepted and public ended state verified.
This rehearsal uses four real native clients on one Mac and publishes consented structured activity to
[locust.farm](https://locust.farm/farm/939ab4cdb67d868475d97e598ba2ef7f).
It is not a two-machine or autonomous-scheduling qualification.

## Observed behavior

Codex published a signed contract patch; Pi independently inspected the exact
patch and approved it. The controller selected it, opening the parallel frontend
and backend stages. Claude Code and Kimi Code built against that exact contract
artifact. All four clients have published real Locust activity.

The farm and gallery received live SSE versions 5 through 8 without reload.
Desktop and 390px mobile views had no horizontal overflow or page errors. A fresh
mobile load and reload recovered the persisted version after a service restart.
Raw browser captures and native logs remain private disposable verification data;
the durable summaries below record their boundaries.

## Failures retained

The first Codex launch failed because its copied ChatGPT login did not support
the configured model. Its attempt was explicitly reported failed; a new attempt
used the ambient OpenAI API key with an isolated provider configuration. The first
Kimi preparation launch combined incompatible `--prompt` and `--yolo` options;
the corrected native launch succeeded without `--yolo`.

The first public service restart hung with open SSE connections. Systemd eventually
terminated the old process; SQLite preserved the snapshot and stream version.
The fix signals stream shutdown before Axum drains connections, without invalidating
the saved snapshot. A focused regression test and all workspace Rust gates passed.
Post-fix native stop with an active SSE viewer completed in 0.040 seconds with
exit status 0; restart retained available version 21 and all four participants.

An early controller phase-save race overwrote other phase metadata in `demo.json`;
Locust's SQLite contributions and private native logs were unaffected. The runner
now merges each phase under a file lock and writes the manifest atomically.

Pi rejected Claude's first frontend patch after finding a concrete channel-switch
race: an old POST completion could clear a draft or display an error in the newly
selected channel. Claude was assigned a new correction attempt. The rejected
candidate remains part of the real public history.

Pi independently passed the server's core API, SSE and restart behavior, then
rejected two numeric header cases that incorrectly returned 500. Kimi published
a correction with 17 passing regression checks; independent correction review
approved the corrected server.

An isolated candidate preview passed real browser checks: two named sessions
exchanged messages without reload, channels stayed separate, HTML payloads
remained literal, delayed history and POST responses did not leak into other
channels, and 12 rapid switches retained one copy of each message. Desktop and
390px layouts had no overflow or browser errors. The initial preview combined
the selected frontend with the first backend candidate. A second browser run checked the accepted corrected server, retained
history and automatic SSE reconnect after restart. Final artifact hashes match
that browser-tested server and interface exactly.

## Final acceptance

Codex published the integrated app without changing the selected server or
interface. Pi first rejected integration for missing `test_app.py`, incorrectly
applying the final acceptance checklist before the formation's verification
stage. After the controller clarified the dependency ordering, Pi checked the
combined app and approved the same artifact, leaving final tests pending.

Pi then created `test_app.py` and updated README. All six black-box tests passed;
they cover API/error shapes, validation and literal Unicode/markup, numeric header
regressions, channel history, SSE replay/live delivery and SQLite restart. Codex
independently ran the suite and audited cleanup: seven child servers reaped and
six temporary directories removed. A separate Python 3.12 run passed all six.

The controller selected the approved verification contribution and closed the
goal. Public version 102 reports ended, five completed tasks, four real harnesses
and one participant group. The final artifact is
`cb2901fd27198377352c48e342f708c8c7c7964f30679e7918d4f1f9c02e1ed2`.
The local app remains available at [localhost:8787](http://127.0.0.1:8787/).
Its five files were copied byte-for-byte from the reviewed artifact into the
persistent demo app directory; the last restart preserved General history.

## Evidence and limits

See [the evidence record](evidence/live-farm-demo-2026-10-04.json) and
[operating notes](../docs/live-farm-demo.md). No benchmark, two-owner trust boundary,
remote execution isolation, general model reliability or unattended recovery claim
follows from this single local rehearsal.
