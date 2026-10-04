# Live farm rehearsal — October 4, 2026

Status: running; final app verification is pending. This rehearsal uses four real
native clients on one Mac and publishes consented structured activity to
[locust.farm](https://locust.farm/farm/939ab4cdb67d868475d97e598ba2ef7f).
It is not a two-machine or autonomous-scheduling qualification.

## Observed behavior

Codex published a signed contract patch; Pi independently inspected the exact
patch and approved it. The controller selected it, opening the parallel frontend
and backend stages. Claude Code and Kimi Code were launched against that exact
contract artifact. All four clients have published real Locust activity.

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

## Evidence and limits

See [the evidence record](evidence/live-farm-demo-2026-10-04.json) and
[operating notes](../docs/live-farm-demo.md). No benchmark, two-owner trust boundary,
remote execution isolation, general model reliability or unattended recovery claim
follows from this single local rehearsal.
