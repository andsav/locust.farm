# Three-agent board trials: evidence

[Findings and measurement definitions](../../role-free-board-2026-10-05.md).
Completed October 5, 2026 PDT, using one Mac and one owner's provider accounts.
These files are retained research evidence, not a product test fixture or a
performance gate.

- [Manifest](manifest.json): pinned executable, source identity, clients,
  permission preparation, private runtime location and original-file hashes.
- [Checksums](SHA256SUMS.json): SHA-256 of every file in this directory except the
  checksum file itself; verify before comparing copies.
- [Setup failures](setup-failures.json): retained aborted Kimi launch and evidence
  collector corrections, separate from completed trials.
- [Signature negative controls](tamper-checks.json): changed signature, header
  and ID all refused.
- [Prompt template](prompt-template.txt), [supplied skill](skill.txt) and
  [runner](runner.py): exact common instructions, eight task texts, grants,
  native launch arguments and twenty-minute limit. The six resolved prompts
  below are the actual per-client input, including absolute local paths.

## Open trial

- [Formation](open/formation.json)
- [Codex prompt](open/prompts/codex.txt), [Claude prompt](open/prompts/claude.txt),
  [Pi prompt](open/prompts/pi.txt)
- [Metrics](open/metrics.json)
- [Canonical signed history](open/after-signed.json)
- [Verified decoded headers](open/verified.json)
- [API event details](open/event-details.json), including decrypted text
- [Initial board](open/before-board.json), [final board](open/after-board.json)
- [Refusals and stale board read](open/selected-cli-receipts.json)
- [Candidate test outcomes](open/candidate-checks.json)

The `open/bundles/` files are exact UTF-8 JSON bytes returned by `blob get`, named
by their signed artifact references. The metrics and candidate checks map each
file to its task, author, contribution and plaintext SHA-256. The artifact
reference addresses goal-sealed storage; it is not the plaintext SHA-256.

## Peer-review trial

- [Formation](peer-review/formation.json)
- [Codex prompt](peer-review/prompts/codex.txt),
  [Claude prompt](peer-review/prompts/claude.txt),
  [Pi prompt](peer-review/prompts/pi.txt)
- [Metrics](peer-review/metrics.json), including all sixteen automatic requests
  and each review's exact preceding request
- [Canonical signed history](peer-review/after-signed.json)
- [Verified decoded headers](peer-review/verified.json)
- [API event details](peer-review/event-details.json)
- [Initial board](peer-review/before-board.json),
  [final board](peer-review/after-board.json)
- [CLI refusals](peer-review/selected-cli-receipts.json)
- [Candidate test outcomes](peer-review/candidate-checks.json)

The `peer-review/bundles/` files use the same exact-byte convention as open.
Canonical headers include automatic effects with diagnostic `at_ms: 0`.
They are not assigned an invented wall-clock time. SQLite insertion positions
and API replay positions are distinct; joins use signed event identifiers.

## Reproduction helpers

[runner.py](runner.py) imports the existing
[live demo controller](../../../scripts/live_farm_demo.py) for isolated profiles,
a local daemon and bound credentials. It replaces that controller's phase
assignments with simultaneous native launches and uniform preparation. Pi's path
names the installation actually found on this Mac; another machine must supply
its installed path. Provider credentials stay local and are never part of these
files. Do not run these helpers against normal user daemon state.

To repeat with real provider calls, build the selected source revision with its
pinned Rust toolchain, create a fresh private state directory, copy the resulting
binary there as `locust`, and run `runner.py prepare STATE open`, followed by
`runner.py run STATE open`. Prepare and run `peer-review` in the same way. Each
formation gets an independent directory under STATE; preparation refuses an
existing manifest. Every process receives its own prompt, profile and session.
The driver does not restart clients, assign work, request reviews or modify
permissions during a trial.

[collect.py](collect.py) obtains complete API views and artifact bytes after
native processes finish, stops that trial's daemon, then reads canonical rows
from its exclusively locked SQLite database. It invokes the verifier at
`STATE/verify-events`. Build that helper with
`python3 research/evidence/role-free-board-2026-10-05/build_verifier.py STATE/verify-events`
after building the workspace. [verify_events.rs](verify_events.rs) calls the
protocol's `Event::decode` for canonical encoding, structure, event-ID and
Ed25519 checks; this is a separate verification pass using Locust's crypto
implementation, not an independent implementation of the protocol.

[measure.py](measure.py) takes `STATE FORMATION` and derives results from verified
headers, checks their bodies/authors/times against API views, and joins refusal
receipts from the scoped CLI logs. [check_candidates.py](check_candidates.py)
materializes each cited bundle, runs its tests and the additional
[oracle](oracle.py), retaining exit codes and complete test output. Neither
measurement helper treats a model's final answer as completion evidence.

For retained public copies, verification needs no daemon or provider: build the
verifier and pass either `open/after-signed.json` or
`peer-review/after-signed.json`. Bundles can be decoded into fresh directories
and checked with `python3 oracle.py TASK DIRECTORY` plus their unittest files.
Full private native transcripts, profiles, databases and credentials remain at
the manifest's local runtime path. The public files contain no signing keys,
provider keys, session secrets or invitation tickets.
