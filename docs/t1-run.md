# T1 CLI run on three Apple Silicon Macs

Date: 2026-10-03. **Status: the three-process local workflow passes on one Mac. The published build and three-Mac run are not yet qualified.** The [build guide](t1-build.md) identifies the candidate; the [release ledger](release-evidence.md) records the evidence boundary. These commands exercise the production CLI, authenticated local API, SQLite store and encrypted Iroh synchronization. They do not require a coding client or MCP.

## Download and start

The first-contact path must download the same published binary and verify its pinned SHA-256 on each Mac before starting it. The source repository is private; a public binary download location requires the owner's choice and publication instruction. A local build copied between machines does not complete this step. The candidate's exact download command will be recorded with its URL, commit and digest after that decision.

Run the verified binary in the foreground on each Mac:

```sh
export LOCUST_HOME="$HOME/.locust-t1"
/path/to/locust --home "$LOCUST_HOME" daemon run
```

Keep that terminal open. In a second terminal, set `LOCUST` to the absolute path of the same verified binary and choose `m1`, `m2` or `m3` for this machine:

```sh
export LOCUST=/absolute/path/to/locust
export LOCUST_HOME="$HOME/.locust-t1"
PARTICIPANT=m1
"$LOCUST" --owner agent enroll "$PARTICIPANT" --manage-goals
export LOCUST_CREDENTIAL="$LOCUST_HOME/agents/$PARTICIPANT.credential"
"$LOCUST" --json status
"$LOCUST" --json doctor
```

Record `"$LOCUST" --version`, `shasum -a 256 "$LOCUST"`, `uname -m` and `sw_vers -productVersion` on every machine. The binary hash and embedded source commit must agree. Enrollment prints the credential's path and public principal key; use the principal key where a command below asks for `M2_PRINCIPAL`.

## Join and complete a task

On M1:

```sh
"$LOCUST" --json goal create --title "Three Mac T1"
GOAL=full_goal_id_from_the_response
"$LOCUST" --json goal invite --goal "$GOAL"
"$LOCUST" --json goal invite --goal "$GOAL"
```

Give one ticket to M2 and the other to M3. Each joins with its own ticket and records the returned goal ID:

```sh
"$LOCUST" --json goal join --ticket 'ticket_for_this_machine'
GOAL=full_goal_id_from_the_response
"$LOCUST" --json goal status --goal "$GOAL"
```

Wait until all three replicas list all three principals and the decrypted goal title. Save the daemon's `peer ... paths` lines; they name the observed direct or relay route without IP addresses. On M1, propose and assign:

```sh
"$LOCUST" --json task propose --goal "$GOAL" "Return the text: T1 task completed."
TASK=full_task_event_id
M2_PRINCIPAL=full_m2_principal_key
"$LOCUST" --json task assign --goal "$GOAL" --task "$TASK" --assignee "$M2_PRINCIPAL"
ASSIGNMENT=full_assignment_event_id
```

On M2, wait until `board --goal "$GOAL"` shows the assignment, then use the same goal and assignment identifiers, authorize execution locally, create the protected execution session, claim and submit:

```sh
"$LOCUST" --owner --json task authorize --goal "$GOAL" --assignment "$ASSIGNMENT"
"$LOCUST" session create "$LOCUST_HOME/sessions/t1.secret"
export LOCUST_SESSION="$LOCUST_HOME/sessions/t1.secret"
"$LOCUST" --json task claim --goal "$GOAL" --assignment "$ASSIGNMENT"
GENERATION=generation_from_the_claim
"$LOCUST" --json task submit --goal "$GOAL" --assignment "$ASSIGNMENT" --generation "$GENERATION" "T1 task completed."
RESULT=full_result_event_id
```

On M1, wait until `event show` returns the received result text, inspect it, then accept it:

```sh
"$LOCUST" --json event show --goal "$GOAL" --event "$RESULT"
"$LOCUST" --json result accept --goal "$GOAL" --result "$RESULT"
"$LOCUST" --json pending --goal "$GOAL"
```

On all three Macs, verify `board --goal "$GOAL"` reports the accepted result and `event show` returns the same decrypted submission. M3 must hold the task, assignment, assignment-accepted event, submission and acceptance despite not taking part in execution.

## Coordinator offline and restart

1. Stop only M1 with `"$LOCUST" --owner daemon stop`.
2. Restart M3's daemon with the same home. This removes its live connection cache while M1 is absent.
3. Add distinct notes on M2 and M3 with `"$LOCUST" --json note add --goal "$GOAL" "message from this machine"`. Verify `notes --goal "$GOAL"` on both machines includes both decrypted notes. Record the M2/M3 route.
4. Restart M1 with the same home and verify it receives both notes.
5. Restart each daemon in turn. Verify the same endpoint identity, principals, membership, accepted task, notes and history remain.
6. Sleep a laptop, wake it, and verify that it reconnects and receives a new note. Process restart is not evidence of OS sleep/wake.

Record all failures before retrying. This run does not cover workspace snapshots, real coding clients, signed installation, member removal or large-content qualification; their later gates remain in the [workstream sequence](workstreams.md).

## Network operation

The daemon defaults to n0 relays and both local multicast and BitTorrent Mainline DHT address lookup. Invitation hints are an optimization; signed member endpoint keys are enough to attempt discovery. Mainline publishes signed endpoint-key-to-relay records, not goal IDs, membership or content. DHT participants see lookup/publication traffic and its network source. Local multicast advertises endpoint keys and contact addresses on the local network. n0 relays see connection metadata and encrypted traffic; they do not receive goal decryption keys.

Operator overrides are `LOCUST_RELAY=none`, `n0`, or one custom relay HTTPS URL (HTTP is allowed only for loopback/localhost); `LOCUST_LOOKUP=none`, `local`, `mainline`, or `all`; and `LOCUST_BIND=<socket address>`. Disabling relays also removes the relay address that the default Mainline publisher advertises. These are explicit operator settings, not conditions a participant must configure for T1. The [network shell](../crates/locust/src/daemon/network.rs) validates them; [transport configuration](../crates/locust-net/src/lib.rs) defines discovery and relay choices.

## Reproduce the local integration check

```sh
python3 scripts/check_t1.py --binary /absolute/path/to/locust
```

The [harness](../scripts/check_t1.py) freezes the selected executable, creates three private temporary homes, runs the sequence through the CLI, and reaps its daemons. It records version/hash, event IDs, observed routes and redacted command responses. `--timeout-seconds` is a qualification watchdog, not a daemon task limit. `--network local` is a diagnostic reproduction with relays disabled and multicast only; it is not the default T1 configuration. The harness does not exercise physical Macs or sleep/wake.
