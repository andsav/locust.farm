# Research evidence appendices

These files preserve local characterization probes and captured results from 2026-10-03. Each names its target revision and distinguishes upstream research from Locust implementation evidence.

- [Lane A review probes](lane-a-review-probes.md): workspace export and unfinished authority/sync reproductions; [capture hashes](lane-a-review-snapshot.json) identify the reviewed working source. See the [review](../lane-a-review-2026-10-03.md) for findings and takeover assessment.

- [Task lifecycle probes](task-probes.md): five store/RPC observations.
- [Raft restart probe](raft-restart-probe.md): duplicate application after an unsnapshotted component restart.
- [hcom validation](hcom-validation.md): isolated suites, scripted-provider native-client results and failure evidence, plus a storage-error characterization.
- [Transport probe measurements](transport-probe-2026-10-03.json): redacted Locust same-host public-relay, custom-relay and direct-path observations; interpreted in the [transport findings](../iroh-transport-probe.md).
- [ecdsa.fail leaderboard analysis](ecdsa-fail-leaderboard-analysis.md): script and output over the public submissions data; interpreted in the [benchmark note](../ecdsa-fail-benchmark.md).

Read the [MoltMesh validation report](../moltmesh-validation.md) or [hcom dissection](../hcom-dissection.md) for interpretation and test limits, and the [research index](../README.md) for findings.
