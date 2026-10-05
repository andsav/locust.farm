# Locust organization TLA+ models

The executable models target the organization protocol subset introduced in
`c88e3bc960de79eb990b3185a653bd982e587ed6`. Start with the
[property and evidence map](organization.md). The
[formal verification plan](../../docs/tla-verification-plan.md) identifies
unmodeled behavior and later proof work. These are bounded exhaustive checks,
not a deductive proof or Rust refinement proof.

- [Organization](Organization.tla): authenticated event delivery, narrow
  governance, tenure cutoffs, pinned definitions/rules, completion review identity,
  and scoped exact selection proofs.
- [Attempt sessions](AttemptSessions.tla): independent attempts, local permission,
  claim generations, principal binding, keyed retries and uncertain commit recovery.
- [Flow effects](FlowEffects.tla): logical effect identity, duplicate signatures,
  durable outbox source, delivery/acknowledgment/start separation and retraction.
- [Case registry](cases.json): exact finite configurations, safety properties,
  requested witnesses and deliberate mutation expectations.

## Run

Use Python 3.12 or newer. Tools download only with `--bootstrap` into ignored
`output/tla/tools/`; [toolchain.json](toolchain.json) pins TLC 1.7.4 and Temurin
21.0.8+9 archives/checksums. The runner verifies the extracted runtime against its
archive rather than accepting only its version string.

```sh
python3 -m unittest discover -s scripts/tests -p test_check_tla.py
python3 scripts/check_tla.py --bootstrap --suite fixtures
python3 scripts/check_tla.py --suite fast
python3 scripts/check_tla.py --suite extended
```

`organization`, `sessions`, and `effects` select each model's cases; `--case <id>`
selects individual cases. `fast` includes organization safety and short directed
witness/mutation checks. `extended` additionally exhausts the general local-session
and effect action systems. Finite identities, transcripts, generation ranges and
event counts are written in each configuration and the model map. They bound a
verification experiment, not product behavior.

There is **no implicit wall-time deadline**. Supply `--timeout <seconds>` only when
you want one; a timeout is incomplete verification and cannot pass. The default
Java heap is 4096 MB, explicitly adjustable using `--memory-mb`. Resource exhaustion
also fails. This heap setting is a checker process resource allocation, not a
Locust history or operation limit.

Each run freezes input files, records source/model/tool hashes and preserves raw
logs and normalized counterexample traces under `output/tla/runs/`. A passing
safety case requires normal completion and an empty queue. A witness or mutation
must violate the exact named property and match the registered state predicates;
parser errors, unrelated violations and incomplete runs do not count.

Checking uses one worker, breadth-first exploration, fingerprint 0 and seed 1.
TLC fingerprint collision estimates are retained. No simulation, symmetry or
coverage instrumentation is used. Witnesses establish specified reachability;
they do not establish fairness or network liveness. Selected results and traces
are tracked in [organization evidence](../evidence/tla/organization/README.md).
The bootstrap CI configuration remains distinct from locally observed checks;
no new remote CI outcome is asserted here.
