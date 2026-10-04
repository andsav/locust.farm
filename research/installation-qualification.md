# Local installation qualification

Date: 2026-10-03. **Status: all ten local native installation, upgrade and
launchd cases passed on the identified final candidate.** This record covers local qualification of the
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
the final-candidate run below additionally passed the distinct-baseline upgrade.

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

## Final native campaign

The [retained result](installation-qualification-evidence.json) records a pass
for all ten scenarios on macOS 26.4 arm64. The native source is
`5bb254d97504209c1ee4277e74c1365c2d8620e0`; executable SHA-256 is
`abe1c0271de5c8fdbd8145d35b6b0932233d02eee7b5957fc99fc3211eac8580` and
manifest SHA-256 is
`3793145c1aa0aa7aae24e8572d4b60683ff8d5205d1ce26202d4d01843c4d8c7`.
The trusted bootstrap is this same explicitly trusted local source build;
trust did not derive from running an unknown candidate's self-verifier.
The harness hash is
`29b88365b33fd533d2a1dfe7373d3a14ac5ff5c43df35eb0c36ba48758558b51`.

The real upgrade installed source
`6757d755b6f3eb515dfa1c451ff2b76ec497f3de` first, created a principal, goal and
note with its installed daemon, then activated the final candidate through the
verified installer. The restarted final daemon retained its endpoint identity
and prior note and passed doctor. Both releases use version 0.1.0; their full
source, manifest and executable identities differ and are recorded. This is
an actual native cross-commit upgrade, not a substituted probe or manifest-only
fixture. The database schema is unchanged between these two commits; this
checks activation and data preservation, not migration between schema versions.

The launchd failure case observed daemon exit code 6 with the synthetic
non-directory home, preserved the owned unit and ownership record, and passed
API readiness/doctor after explicit bootout and fixture repair. The normal
lifecycle refused removal while both running and loaded without a process,
then completed explicit stop/removal and retained the original daemon note
through restart. Test signing keys and every synthetic service were cleaned up.

| Measurement | Observed value |
|---|---:|
| Installed executable | 13,678,896 bytes |
| Fresh direct spawn to authenticated status | 69.7 ms |
| Direct restart to authenticated status | 66.8 ms |
| Native launchd start to authenticated status | 123.9 ms |
| Idle RSS, three samples one second apart | 15,745,024 bytes each |
| Accumulated CPU delta over 2.017 seconds | 0.00 seconds at `ps` precision |
| Network bytes | Unmeasured |

These measurements ran concurrently with the operational qualification on the
same host. The zero CPU counter delta is a short sampled observation, not a
claim of zero CPU work or a benchmark threshold. This campaign does not qualify
Linux systemd, another machine/account, production signing, publication or
third-party client operation.

The raw redacted transcript is kept under ignored
`output/installation/20261004T065745Z-494b989f/`; its hash, the original summary
hash and the complete compact result are retained in the tracked evidence file.
