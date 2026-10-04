# T1 CLI run: protocol 1 on two Apple Silicon Macs

> **Historical pre-organization document.** Its protocol-1 commands, source
> snapshots and qualification claims do not apply to API 2 / protocol 2. Read
> [the current manual](guide/overview.md) and
> [implementation status](formations-status.md) for current behavior.
> No legacy reader, migration or old-runtime support is provided.

Date: 2026-10-03. **Status: current commands target the remediated API-1/protocol-1 build. Physical qualification of that build remains pending.** The historical protocol-0 run on two Macs recorded joining, a decrypted note and assignment readiness; it stopped before claim/submission. Its [worker findings](../research/t1-m2-smoke-2026-10-03.md) and [release ledger](release-evidence.md) remain historical evidence. Publication is deferred. The [build guide](t1-build.md) and [remediation record](../research/t1-remediation.md) identify current source and verification. These commands use the production CLI, local API, SQLite store and encrypted Iroh synchronization; they do not require a coding client or MCP.

Version 1 refuses version-0 peers, tickets and stored events. Preserve the previous binaries and `~/.locust-t1` homes. This sequence uses a fresh `~/.locust-t1-v1` on each Mac; it does not migrate the earlier goal.
## Verify and start the candidate

The current [identified local candidate](packaging.md) is `5bb254d` and includes
T2. Copy the same archive to both Macs using AirDrop or a shared folder and
compare its SHA-256 with that independently provided record before extraction.
The archive contains `locust`, `skills/locust/SKILL.md` and `manifest.json`.
It is a locally built candidate with test-only signing evidence, not a public
publisher-trusted release. Do not copy a daemon home: each Mac creates its own
identity and state. No rebuild is needed.

The older [T1 bundles](t1-build.md) instead contain `locust`, `SHA256SUMS` and
`metadata.json`. Their historical checks remain valid for their recorded bytes;
they do not include the current installed-client implementation.

Set `LOCUST` to the absolute path of the locally trusted candidate executable.
Check its SHA-256 against the current candidate record before starting it:

```sh
export LOCUST=/absolute/path/to/locust
cd "$(dirname "$LOCUST")"
shasum -a 256 "$LOCUST"
chmod u+x "$LOCUST"
"$LOCUST" --version
```

For the current candidate the hash must be
`abe1c0271de5c8fdbd8145d35b6b0932233d02eee7b5957fc99fc3211eac8580`
and the version must be `locust 0.1.0 (5bb254d97504) api 1 protocol 1`.
Both Macs must agree. The old `3422c7b` candidate is protocol 0 and cannot be used
for this sequence. Signed installation and client configuration follow the
separate [local installation procedure](installation.md); this CLI sequence
does not by itself qualify that experience. Once both agree, start the daemon:

```sh
export LOCUST_HOME="$HOME/.locust-t1-v1"
"$LOCUST" --home "$LOCUST_HOME" daemon run
```

Keep the daemon terminal open. In a second terminal, set `LOCUST` to the same executable and use `m1` on the first Mac, `m2` on the second:

```sh
export LOCUST=/absolute/path/to/locust
export LOCUST_HOME="$HOME/.locust-t1-v1"
PARTICIPANT=m1
"$LOCUST" --owner agent enroll "$PARTICIPANT" --manage-goals
export LOCUST_CREDENTIAL="$LOCUST_HOME/agents/$PARTICIPANT.credential"
"$LOCUST" --json status
"$LOCUST" --json doctor
```

Record `"$LOCUST" --version`, `shasum -a 256 "$LOCUST"`, `uname -m` and `sw_vers -productVersion` on both machines. The binary hash and embedded source commit must agree. Enrollment prints the credential's path and public principal key; use M2's principal key where a command below asks for `M2_PRINCIPAL`. Enroll once per fresh home; keep that same home and credential when restarting. A retry with different grants returns `conflict`; change an existing principal explicitly with `"$LOCUST" --owner agent grant --agent FULL_PRINCIPAL_KEY --manage-goals true`.

## Join and complete a task

On M1:

```sh
"$LOCUST" --json goal create --title "Two Mac T1"
GOAL=full_goal_id_from_the_response
INVITE_EXPIRES_MS="$(python3 -c 'import time; print(time.time_ns() // 1_000_000 + 3_600_000)')"
"$LOCUST" --json goal invite --goal "$GOAL" --expires-ms "$INVITE_EXPIRES_MS"
```

The expiry is one hour from M1's clock, expressed as absolute Unix milliseconds. Give the full ticket to M2 through the chosen handoff channel. On M2, run the join command, paste the ticket on standard input, then press Control-D; it stays out of the process argument list. A private ticket file can instead be redirected with `< /absolute/path/to/private-ticket.txt`. Record the returned goal ID:

```sh
"$LOCUST" --json goal join --ticket -
GOAL=full_goal_id_from_the_response
"$LOCUST" --json goal status --goal "$GOAL"
```

Run `status` and `goal status --goal "$GOAL"` on both Macs until each lists both admitted principals and the decrypted goal title. A pending join may return `unavailable` for goal reads; `status` still shows the local attempt. If it becomes `refused`, ask M1 for a fresh unexpired invitation and repeat the stdin join. Each ticket is single-recipient; tickets for the same goal share a long prefix, so copy the full value. Save the daemon's `peer ... paths` lines; they name the observed direct or relay route without IP addresses. On M1, propose and assign:

```sh
"$LOCUST" --json task propose --goal "$GOAL" "Return the text: T1 task completed."
TASK=full_task_event_id
M2_PRINCIPAL=full_m2_principal_key
"$LOCUST" --json task assign --goal "$GOAL" --task "$TASK" --assignee "$M2_PRINCIPAL"
ASSIGNMENT=full_assignment_event_id
```

Copy the goal and assignment identifiers to M2. On M2, wait until `board --goal "$GOAL"` shows the assignment, then authorize execution locally, create the protected execution session, claim and submit:

```sh
ASSIGNMENT=full_assignment_event_id_from_m1
"$LOCUST" --owner --json task authorize --goal "$GOAL" --assignment "$ASSIGNMENT"
"$LOCUST" session create "$LOCUST_HOME/sessions/t1.secret"
export LOCUST_SESSION="$LOCUST_HOME/sessions/t1.secret"
"$LOCUST" --json task claim --goal "$GOAL" --assignment "$ASSIGNMENT"
GENERATION=generation_from_the_claim
"$LOCUST" --json task submit --goal "$GOAL" --assignment "$ASSIGNMENT" --generation "$GENERATION" "T1 task completed."
RESULT=full_result_event_id
```

Copy the result identifier to M1 and set `RESULT` there. On M1, wait until `event show` returns the received result text, inspect it, then accept it:

```sh
RESULT=full_result_event_id_from_m2
"$LOCUST" --json event show --goal "$GOAL" --event "$RESULT"
"$LOCUST" --json result accept --goal "$GOAL" --result "$RESULT"
"$LOCUST" --json pending --goal "$GOAL"
```

On both Macs, verify `board --goal "$GOAL"` reports the accepted result and `event show --goal "$GOAL" --event "$RESULT"` returns the same decrypted submission.

## Offline writes, catch-up, restart and sleep/wake

1. Stop only M1 with `"$LOCUST" --owner daemon stop`.
2. On M2, run `"$LOCUST" --json note add --goal "$GOAL" "M2 wrote this while M1 was offline"`, then `"$LOCUST" --json notes --goal "$GOAL"`. Verify the note is readable locally while M1 is stopped.
3. Restart M1 with `"$LOCUST" --home "$LOCUST_HOME" daemon run` in its daemon terminal. On M1, run `"$LOCUST" --json notes --goal "$GOAL"` until the same decrypted note arrives. Record the reconnect route and note event identifier.
4. Restart M2, then M1, one at a time. Stop with `"$LOCUST" --owner daemon stop` in its command terminal, then run `"$LOCUST" --home "$LOCUST_HOME" daemon run` in its daemon terminal, using the original home. After each restart, verify `status`, `goal status`, `board` and `notes` preserve the endpoint identity, principal, membership, accepted task and content.
5. Leave both daemons running and sleep M2 through macOS. While M2 is asleep, add a distinct note on M1. Wake M2 and verify it reconnects and receives that note. Record the sleep/wake steps and the note identifier; process restart does not substitute for OS sleep/wake.

With two daemon identities, step 2 proves local work continues while the other peer is absent, and step 3 proves catch-up. There is only one remaining peer while M1 is stopped, so this run cannot prove exchange between two surviving peers without the coordinator. That requires the optional extension below.

Record all failures before retrying. This run does not cover workspace snapshots, real coding clients, signed installation, member removal or large-content qualification; their later gates remain in the [workstream sequence](workstreams.md).

## Optional third participant

When a third Mac is available, copy the same bundle and repeat setup with participant `m3` and a fresh home. On M1, issue another `goal invite` for the existing goal; M3 joins with that ticket. Verify all three replicas list all three principals, and M3 obtains the earlier task, assignment, assignment-accepted event, submission and acceptance despite not taking part in execution.

Then exercise exchange without the coordinator:

1. Stop only M1's daemon.
2. Restart M3's daemon with its existing home to clear its live connection cache.
3. Add distinct notes on M2 and M3. Verify each receives the other's decrypted note and record the M2/M3 route.
4. Restart M1 and verify it catches up with both notes.
5. Restart M3 once more and verify it retains its identity, membership, task and content.

A third daemon can instead run on one of the two Macs with a separate `LOCUST_HOME`, enrollment, credential and session, and its own terminals. That supplies three-peer evidence on two physical machines. Record that topology explicitly; it does not establish behavior on three physical Macs. Neither optional extension is a prerequisite for starting the two-Mac run.

## Network operation

The daemon defaults to n0 relays and both local multicast and BitTorrent Mainline DHT address lookup. Invitation hints are an optimization; signed member endpoint keys are enough to attempt discovery. Mainline publishes signed endpoint-key-to-relay records, not goal IDs, membership or content. DHT participants see lookup/publication traffic and its network source. Local multicast advertises endpoint keys and contact addresses on the local network. n0 relays see connection metadata and encrypted traffic; they do not receive goal decryption keys.

Operator overrides are `LOCUST_RELAY=none`, `n0`, or one custom relay HTTPS URL (HTTP is allowed only for loopback/localhost); `LOCUST_LOOKUP=none`, `local`, `mainline`, or `all`; and `LOCUST_BIND=<socket address>`. Disabling relays also removes the relay address that the default Mainline publisher advertises. These are explicit operator settings, not conditions a participant must configure for T1. The [network shell](../crates/locust/src/daemon/network.rs) validates them; [transport configuration](../crates/locust-net/src/lib.rs) defines discovery and relay choices.

## Reproduce the local integration check

```sh
python3 scripts/check_t1.py --binary /absolute/path/to/locust
```

The [harness](../scripts/check_t1.py) freezes the selected executable, creates three private temporary homes, runs the sequence through the CLI, and reaps its daemons. It records version/hash, event IDs, observed routes and redacted command responses. `--timeout-seconds` is a qualification watchdog, not a daemon task limit. `--network local` is a diagnostic reproduction with relays disabled and multicast only; it is not the default T1 configuration. The harness does not exercise physical Macs or sleep/wake.
