# Members not syncing on the local-network profile while the host is away

Investigation, 7 October 2026. Root cause found and verified. The harnesses
gained a precondition check; no Rust behavior changed.

Phase R6's verification left two supplemental failures open
([R6 build notes](v2-phase-r6-build-notes-2026-10-06.md), "Supplemental
harnesses"): `check_t1.py --network local` stopped after ten checkpoints when
the two members had to exchange notes with the host stopped, and
`simulate_machines/run.py --quick` failed all three scenarios. The promise at
stake is that members keep syncing with each other while the host's computer
is off.

## Answer

- **Neither failure reproduces on current main** (`a19a395`, with G1) when the
  harnesses run from this session. Both passed; the exact R6 binary passes
  too. Repeated runs later showed an unrelated, intermittent failure further
  on in the `crash` scenario, in about half of its runs, before G1 as well
  (last section).
- **The cause is the environment R6 ran in, not the product.** R6's harnesses
  were started from the Codex app, and macOS denies that app local-network
  multicast. Every daemon the harness started inherited the denial. The
  `local` and `lan` profiles turn the relay and the wide-area lookup off, so
  the only way two members who are not the host can find each other is
  multicast lookup (mDNS). With it denied, members reached the host through
  the address in their ticket and never reached each other.
- **With multicast denied, current main fails exactly as R6 did**: the same
  steps, the same messages, about the same times, and no network path between
  two members in any run.
- **G1 played no part.** Current main includes G1 and passes; the binary R6
  used predates G1's commits and passes too. A restart is not a restore: the
  only `guard` entries any run here showed were G1's `admitted` hold on a member
  just after it joined, as designed, and none at the host-offline step.
- **The harness was right that nothing synced, but wrong about why.** It ran a
  profile its environment could not support and reported a product failure.
  It now checks that it can use local-network multicast before it starts a
  daemon, and otherwise stops at once with an `environment:` failure. No
  check, timeout or network profile changed.

## How a member learns another member's address

A daemon dials a peer by its endpoint id plus whatever contact hints it has
stored. It stores hints from exactly one source:

1. **The ticket.** A joiner stores the inviting daemon's hints when it joins
   (`crates/locust-core/src/node/requests/invitations.rs`, the
   `records::put(Space::Peer, invitation.endpoint ...)` write in the join plan).
   `Host::hints` reads them back (`crates/locust-core/src/node/peers.rs`) and the
   sync driver passes them with every `PeerOutput::Open`
   (`crates/locust-core/src/sync/driver.rs`, `poll`). Nothing else writes
   `Space::Peer` hints, so for every member except the host the hints are
   empty. The host-safety plan records the same reading: "a member's contact
   hints are stored only from a ticket, so dials to other members already rely
   on the transport finding an endpoint by its id"
   ([plan](../docs/host-safety-and-ending-plan.md)).

Everything else is lookup by endpoint id inside the transport
([locust-net](../crates/locust-net/src/lib.rs), `Endpoint::bind`):

2. **Local-network lookup** (mDNS through `iroh-mdns-address-lookup` 0.6.0),
   when `Lookup::local_network` is on.
3. **Wide-area lookup** (Mainline DHT), when `Lookup::mainline` is on, together
   with the n0 relays the published record points to.
4. **Live connections.** Iroh remembers the address an open connection came
   from, in memory only.

`LOCUST_RELAY=none LOCUST_LOOKUP=local`, the harnesses' `local` and `lan`
profiles ([network.rs](../crates/locust/src/daemon/network.rs), `bind`),
leaves only 1, 2 and 4. No frame between daemons carries one member's address
to another.

Every start binds the IP transport to port 0 (iroh 1.3.0,
`socket/transports.rs`, `default_ipv4` and `default_ipv6`), so a restarted
daemon comes back on a new port. The ticket's hints for a restarted host and
other daemons' in-memory addresses for any restarted daemon are stale from
then on.

## Why the dial and the exchange never happen

The driver keeps a (goal, endpoint) pair for every other active member and
opens an exchange when one is due, retrying a failed endpoint after 1 second,
doubling to 60 seconds. For a member that is not the host the shell calls
`Endpoint::connect(peer, &[])`. With multicast denied the mDNS lookup never
answers; Iroh gives up after the lookup's 10 seconds and the connect fails
(`transport: Connect`). The shell reports `OpenFailed`, the driver backs off
and tries again, indefinitely. No exchange starts.

The host is different only while it keeps its port: members dial it with the
ticket's hints. Hence the shape of the R6 failures:

- `check_t1.py --network local` and the `three-machines` scenario: membership,
  the task and the first replication all went through the host. Once the host
  stopped, the two members had no address for each other.
- `two-machines-complete`: the host restarted on a new port. The member's
  ticket hints were stale and the host had never stored the member's address,
  so neither could reach the other.
- `crash`: the member killed with `kill -9` came back on a new port. It
  reached the host through the ticket, which is why its note reached everyone
  in 0.25 seconds through the host, but it never reached the third member, so
  "connected to the others again" timed out.

A restarted member reconnects to other members only through a lookup: its own
address book is gone, it holds hints for the host only, and the others hold
its old port.

## Evidence

Same Mac (macOS 26.4, arm64), Python 3.13.7. The machine was shared with other
sessions' builds and daemons, at load averages of about 6 to 14; every
passing run below finished far inside its deadlines, and every failing run
failed by a deadline that no load explains (no path between members ever
appeared). Logs and transcripts are under the worktree's ignored `output/`.

| Run | Started from | Binary | Result | Paths between two members |
| --- | --- | --- | --- | --- |
| R6 `check_t1.py --network local` | Codex app | `1b21c3f7cfad-dirty`, SHA-256 `00fdb99a…5169f` | Failed after 10 checkpoints | 0 of 4 route facts |
| R6 `run.py --quick` (`lan`) | Codex app | same | All three failed: 95.2 s, 95.5 s, 182.3 s | 0 in every scenario |
| R6 `check_operations.py`, default network | Codex app | same | Passed | 22 route facts between members, all relay; none direct |
| `check_t1.py --network local` | this session | main `a19a395` | Passed, 21 checkpoints | 8 of 24, all direct |
| `run.py --quick` (`lan`) | this session | main `a19a395` | All three passed: 7.5 s, 11.9 s, 31.7 s | 8 (three-machines), 29 (crash) |
| `check_t1.py --network local` | this session | the exact R6 binary | Passed, 21 checkpoints | 12 of 26 |
| `check_t1.py --network local`, mDNS denied | this session | main `a19a395` | Failed at the R6 step after the same 10 checkpoints | 0 of 4 |
| `run.py --quick` (`lan`), mDNS denied | this session | main `a19a395` | All three failed with R6's messages: 92.3 s, 93.8 s, 184.2 s | 0 in every scenario |

"mDNS denied" means the daemons ran under `sandbox-exec` with
`(allow default) (deny network-outbound (remote udp "*:5353"))`: unicast works
and only mDNS sends are refused, as with the Codex app's setting below. For the
simulation the profile wrapped only the daemon binary, because a sandboxed
Python may not run the harness's `ps`. The T1 failure text differs only in
which call the 60-second wait expired in (`M3 CLI command timed out` rather
than `qualification wait timed out`).

At the Rust level, the ignored transport test
`mdns_finds_a_peer_by_key_without_contact_hints`
([tests.rs](../crates/locust-net/src/tests.rs)) passes in 0.81 s here and,
under the same profile, fails after 10.04 s with `transport: Connect`.

Why the R6 runs had no multicast:

- The unified log for the R6 window (6 October, 19:40 to 20:00 local time)
  holds 332 lines of `LocalNetwork: found bundle id com.openai.codex`, one
  Local Network privacy check per `locust` process the harnesses started.
  The daemons started from this session raised no such check.
- `/Library/Preferences/com.apple.networkextension.plist` holds the per-app
  Local Network decisions. For `com.openai.codex` it has `DenyMulticast` true
  (and `MulticastPreferenceSet` true, so the decision was made), with
  `DenyAll` false: unicast allowed, multicast denied. That matches what the
  runs show: members reached the host's ticket address but never found each
  other. For `com.anthropic.claude-code`, which this session's processes are
  attributed to, `DenyMulticast` is false.
- In the shared checkout's `output/`, every relay-free local run that got as
  far as the host-offline step without another error, from 4 and 6 October,
  shows no path between two members; the 4 October simulation that passed
  shows them. The same code passes or fails depending on which app started
  it.

## The harness change

[check_t1.py](../scripts/check_t1.py) gains `local_multicast`, which joins the
mDNS group on port 5353 the way the daemons' lookup does, sends one standard
mDNS question for the service Iroh uses, and requires its own copy back over
multicast loopback within 2 seconds. A relay-free local run calls
`require_local_multicast` before any daemon starts: `check_t1.py --network
local` and `check_operations.py --network local` in `Qualification.execute`,
and every simulation scenario whose profile is `lan`
([simlib.py](../scripts/simulate_machines/simlib.py),
`Cluster.execute_scenario`). The result is recorded in the summary as
`transport.local_multicast`. When it fails, the run fails with
`environment: local-network multicast is unavailable to this run: <reason>`
and says how to allow it. A run that would have passed still passes; a run
that cannot exercise its profile still fails, at once and with the reason.

Unit tests in [test_t1_harness.py](../scripts/tests/test_t1_harness.py) and
[test_simulate_machines.py](../scripts/tests/test_simulate_machines.py) cover
the query bytes, a denied send (`EHOSTUNREACH`, as Local Network privacy
reports it, and `EPERM`, as a sandbox does), a dropped loopback copy, a local
run failing before any daemon, and a default-network run not probing.

| Harness, same binary (main `a19a395`) | Before the change | After the change |
| --- | --- | --- |
| `check_t1.py --network local` | Passed, 21 checkpoints | Passed, 21 checkpoints; `local_multicast: available` |
| `run.py --quick` | Passed, 3 of 3 | `two-machines-complete` and `three-machines` passed; `crash` failed at a later, unrelated step (below) |
| `check_t1.py --network local`, mDNS denied | Failed after 60 s at the host-offline step | Failed in under a second, before any daemon started: `environment: … EPERM (Operation not permitted)` |

## What a person could meet, and options

With the default settings nothing here stops sync: members that cannot use
multicast still find each other through the Mainline lookup and the relays,
as R6's default-network run did (relay paths only between members). The
failure needs both relays off and multicast unavailable, for example a daemon
whose launching app or service is denied Local Network access on macOS, Wi-Fi
that isolates clients, or many office networks, with `LOCUST_RELAY=none`. Then
members other than the host cannot find each other, nobody finds a restarted
host, and nothing tells the person why: `doctor` and `status` report the
lookup as configured, not as working.

Neither J1 nor E2 changes how members learn addresses. J1 rewrites admission
of callers that are not members, dial limits, the 15-minute retry for
refused computers and a relay-only mode
([joinable farms plan](../docs/joinable-farms-plan.md), J1); E2 is about
members leaving. Options, none built:

1. **Say so (fits J1).** J1's seventh behaviour adds transport facts to
   `doctor --json`, among them "local lookup". Reporting whether a multicast
   send succeeds, not only whether the lookup is on, would tell a person that
   macOS (or the network) is blocking local discovery and which setting to
   change. Cheap; changes nothing on the wire.
2. **Remember addresses.** Store each member's observed direct addresses from
   its live connection paths under `Space::Peer`, not only the ticket's. A
   restarted daemon could then redial members that kept their port. Does not
   help two members that never met directly, nor a peer that moved.
3. **Keep the port.** Rebind the last port at start, falling back to a new
   one if it is taken. Ticket hints and remembered addresses would survive
   restarts. Pairs with 2.
4. **Share contact hints between members.** In an admitted exchange, each side
   could send the current hints it holds for the goal's other members, from
   authenticated peers only, bounded, used as hints and never as membership.
   This alone makes members that met only through the host find each other
   with no multicast and no relay. It is a new frame or field, so a protocol
   version change, and it tells every member the others' IP addresses, which
   direct connections already reveal and J1's relay-only mode exists to
   avoid. It needs its own plan.

## Also found: a new member's first note refused after a restart

Re-running `run.py --quick` showed a separate, intermittent failure in the
`crash` scenario, at its `join_kill_joiner` step. That step comes after all
three points where R6's runs stopped, and every failing run passed those
first. A new member is killed with `kill -9` just after `goal join` answers,
restarted, and seen as admitted, with the title, on all four computers. Its
first `contribution publish` about 0.3 seconds later is refused: `conflict`,
"the candidate cannot be applied yet (the goal's state). Nothing to change;
pick other work." Just after joining its status showed G1's `admitted` hold;
by the refused post the hold had ended, its `guard` was empty and it had
completed no exchange with any peer (`last_sync_ms` null for all three). The
refusal is the trial replay in `allowed_keeping`
([access.rs](../crates/locust-core/src/node/access.rs)):
the signed candidate came out neither effective nor excluded, so it waits on
something the new member does not hold yet. Which dependency it waits on was
not established; nor whether the same post succeeds a moment later.

| Binary | `crash` runs | Failed at `join_kill_joiner` |
| --- | --- | --- |
| main `a19a395` | 15 | 7 |
| `28c6425`, the commit before G1 | 8 | 4 |
| the R6 binary, `1b21c3f7cfad-dirty` | 5 | 0 |

G1 is not its cause: it fails as often on the commit before G1, which has no
hold. G1's `admitted` hold does not prevent it either. Whether it appeared
between the R6 binary and `28c6425` or the R6 binary was lucky in five runs
was not settled. It is left for a separate change; the network
cause above does not touch it. A person would meet it as a fresh member whose
first post is refused with a message saying nothing will change.
