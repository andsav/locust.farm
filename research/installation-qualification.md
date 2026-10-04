# Local installation qualification

Date: 2026-10-03. **Status: repeatable harness implemented; native release and
service campaign pending.** This record covers local qualification of the
[package format](../docs/packaging.md), trusted bootstrap, installer and
installed daemon. It is not production signing, publication, cross-platform,
physical-machine or independent-account evidence.

## Execution and trust boundary

The [harness](../scripts/check_installation.py) takes separate bootstrap and candidate inputs plus an
optional earlier native bundle for upgrade qualification:

```sh
python3 scripts/check_installation.py \
  --bootstrap /absolute/path/to/explicitly-trusted/locust \
  --bundle /absolute/path/to/extracted-unsigned-candidate \
  --baseline-bundle /absolute/path/to/extracted-earlier-commit \
  --timeout-seconds 60 --sample-interval 1 --samples 3
```

The bootstrap is explicitly trusted to execute. The extracted candidate is
copied as inert fixed-name files; manifest paths never select additional
inputs. The harness refuses symlink payloads, directory aliases below the
selected bundle, and multiply linked files. It copies no ambient credential,
environment or profile files. It generates disposable test-only Ed25519 keys
inside a private ignored output directory, signs the private candidate copy
and withdrawal registries through the bootstrap CLI, and removes those keys
when the run ends. This test key never becomes a production trust root.

The candidate is first executed by the trusted installer's verified version
probe, then through `prefix/current/locust`. Its `--version` response must agree
with its signed manifest; unit-test probe substitution is not used here. The
original unsigned bundle is never executed or modified by the harness. This is
a command-selection policy, not independent execution tracing. Candidate,
manifest, bootstrap and harness hashes are recorded separately.

All user-home, XDG, temporary and daemon paths belong to one disposable
synthetic fixture. The direct daemon uses disabled relay/discovery and an
explicit loopback bind. Port mapping is not independently denied, so this
configuration alone does not prove zero external traffic. Native service
qualification checks that rendered HOME/XDG explicitly name the selected profile;
an explicit daemon `--home` alone does not establish ambient HOME isolation.

## Enforcing scenarios

| Scenario | Required observation |
|---|---|
| Signature and payload checks | Changed detached signature and changed skill bytes are refused before install |
| Reviewed plan | Planning creates no target; a wrong/stale plan hash cannot activate a release |
| Native install and repeat | Real verified version probe succeeds; installed binary hash equals the candidate; repeat apply reports unchanged |
| Withdrawal policy | Older registry, same-sequence replacement, dropped prior withdrawal and a withdrawn target are refused |
| Trust continuity | A separately valid test signature from another key cannot replace an existing installation's trust key |
| Real startup failure | Correctly test-signed version mismatch and unstartable executable fail without changing the active release |
| Partial staging recovery | A preexisting incomplete staging directory is preserved while a complete verified release is installed; this fixture does not claim a timed process interruption |
| Fresh database and restart | Installed daemon creates a principal, goal and note; doctor passes; endpoint/principal/note survive restart |
| Uninstall preservation | Daemon files are unchanged; modified installed skill and unrelated user file remain; trust rollback protection survives uninstall |
| Clean software removal | An unchanged owned release is removed from a separate disposable prefix |
| Native service ownership | Apply/repeat, insecure-log preflight refusal with unchanged ownership, real launchd start plus authenticated API readiness and doctor, running/loaded unit removal refusals, explicit stop/remove |
| Native process failure and retry | A dedicated daemon-home is occupied by a synthetic file; launchd observes nonzero process exit and owned logs while API readiness fails; unit/record remain; bootout and fixture repair permit verified retry |
| Cross-commit upgrade | Optional distinct baseline bundle is test-signed and installed, creates goal/note/identity, stops, upgrades into candidate, and restarts with matching data/identity and healthy doctor; omission is explicitly not run |

## Native service handoff finding

A preparatory macOS 26.4 arm64 run used the clean native baseline
`6757d755b6f3eb515dfa1c451ff2b76ec497f3de` as candidate and an explicitly
trusted current-source debug bootstrap. It reproduced an asynchronous launchd
handoff: `bootout` succeeded while an immediate manager query still reported
the owned job as loaded. The service CLI correctly returned `unavailable`
with `service remains loaded after stop`; a subsequent exact-label query
returned no such service. This was a transient manager observation, not a
lost ownership record or a permanently registered service.

The harness now records only the two specific native transient diagnostics
(`service has not reached running state` and `service remains loaded after
stop`) and polls under its explicit test deadline. Start additionally requires
authenticated daemon readiness and doctor; stop requires the owned manager
state to become stopped before removal. Other errors, a success envelope with
the wrong state, and a job that remains loaded cannot qualify. This adds no
daemon execution limit and does not reinterpret an accepted manager command
as application readiness. The preparatory retry passed all nine non-upgrade
cases, including actual daemon-process failure/retry and exact owned cleanup;
final-candidate and distinct-baseline upgrade evidence remains pending.

## Measurements and limitations

The harness records the actual installed executable size and elapsed time from
process spawn to a successful authenticated status containing an endpoint. It
records fresh and restarted daemon startup separately. Idle RSS is sampled
with `ps` at the explicit interval/count supplied by the operator. CPU use is
the difference between cumulative process CPU counters divided by measured
wall time, expressed as a percentage of one core. RSS is a sampled resident
set, not maximum lifetime or transfer memory; zero/negative RSS is invalid, while
positive values are measurements without a pass/fail performance threshold.
Startup background work may still affect this short idle interval. Network byte
usage is explicitly unmeasured.

The [harness assertion tests](../scripts/tests/test_installation.py) cover
input isolation, trust-role separation, CPU-counter parsing and expected-error
checks that cannot silently pass on success, path-specific probe failure diagnostics
and directory/mode/symlink preservation fingerprints. Run them with:

```sh
python3 -m unittest discover -s scripts/tests -p test_installation.py
```

The launchd campaign uses only a label derived from its disposable daemon home.
It retains the fixture and reports failure if owned-service cleanup cannot finish;
it does not delete a profile under a still-registered job. A missing GUI domain
is explicitly not run, not passed. Native Linux systemd remains a separate gate.

Raw redacted CLI evidence and resource observations are saved under ignored
`output/installation/`. A reviewed compact record and identified native results
will be retained here after the campaign. No native installer, service or
resource result has yet been claimed by this document.
