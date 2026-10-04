# Four-client qualification findings

Date: 2026-10-03 (America/Los_Angeles). **Status: measured client/fixture behavior; no complete Locust client workflow is qualified.** The [runbook](../docs/client-qualification.md) gives reproduction steps and the [ledger](../docs/release-evidence.md) records open release gates.

## Method and sources

The actual installed executables communicate with an original scripted provider on loopback and the Rust MCP fixture through generated registration. All authentication material is dummy and profiles are private/disposable. The OS network guard is tested before calls. The scripted server implements only the selected OpenAI Responses, Anthropic Messages and OpenAI Chat Completions responses; it never calls a real provider. Tool success requires both an MCP completion and a matching authenticated Unix-socket receipt.

Client interfaces were checked against installed help and primary sources: [Factory headless execution](https://docs.factory.com/droid-exec/overview), [Pi MCP](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/mcp.md), [Pi model configuration](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/models.md), and the installed Codex/Claude argument parsers. Pi 1.0.1 came from `@earendil-works/pi-coding-agent` in ignored local output with package scripts disabled; no global package or owner profile was changed. Upstream documentation is context, not proof for another binary version.

## Observations

1. **Default noninteractive policy differs by client and tool.** A write tool has mutating annotations, so read-only success cannot establish write behavior. Explicit policy denial is retained rather than changed to a successful execution or hidden by the permissive run.
2. **Tool exposure differs even with MCP registration.** Codex used a namespaced tool declaration; Droid discovered deferred tools through `ToolSearch`; Pi routed calls through its declared `codemode` tool. The fixture observes resulting calls rather than assuming a flat tool list.
3. **The client must actually continue its old session.** A reused file path alone would falsely pass if Pi replaced the file with a fresh session. The harness now compares the native header identifier and requires the previous bytes to remain a prefix of the longer history.
4. **Leader exit is insufficient cleanup evidence.** Pi can leave a bridge outside the client's process group. The harness observes the fixture's PID, confirms its executable and unique per-run receipt path before signaling, and verifies both the process group and owned child are gone.
5. **Isolation changes must be visible in the result.** Claude's `--bare` is an explicit opt-in; Pi has no added permission extension or permissive flag. Their results are labeled with those exact limitations. A permissive test is not evidence of the default interactive experience.

## Measured results

The results in this section are historical. The [remediation rerun](evidence/client-qualification-remediation-2026-10-03.json) below supersedes their interruption and cleanup evidence; the earlier harness could terminate descendants itself before calling cleanup successful.

The [corrected four-client record](evidence/client-qualification-corrected-2026-10-03.json) retains exact client/probe hashes, harness source hashes, policy labels, OS/architecture, protocol receipts and outcomes. Implementation: `d45a3f5` plus the Factory backend-fixture correction `0e7b150`; bridge configuration is `87f8a42` with shared-contract constants in `d4dbe7c`. All runs used a caller-selected 30-second budget per operation and cleanup on macOS arm64. The corrected harness returned exit 0, and all 54 Python tests passed. Whole-workspace Rust formatting, Clippy with warnings denied and tests also passed after the revision-2/store changes landed.

| Actual client | Default headless read / write | Lifecycle read/write/wait/interruption | Native resume and bridge restart |
|---|---|---|---|
| Codex 0.153.4 | Completed / completed | Passed with explicit permissive flags | Passed, same native session |
| Claude Code 2.1.280 | Denied / denied | Passed with explicit permissive flags and `--bare` | Passed, same native session, `--bare` |
| Factory Droid 0.218.1 | Completed / denied | Passed with explicit permissive flag | Passed with scripted Factory session lookup, same native session |
| Pi 1.0.1 | Completed / completed | Passed under default policy, no permission extension installed | Passed, same native session and extended history |

Denial rows mean successful execution was blocked; the report marks that assertion `not_run`, not `pass`. No interactive approval was supplied. The OS guard allowed loopback and returned EPERM/EACCES for an external TEST-NET address for all four clients. Every temporary runtime profile was removed; owned-process cleanup was checked and any forced cleanup is explicit in the record.

## Remediation rerun

Commit `0391da8` corrects interruption and cleanup qualification. SIGINT targets the client leader only. Success requires natural exit of the owned session and receipt-identified bridge processes; forced cleanup is a failed lifecycle assertion. Same-session process-group changes are tracked, and session membership is rechecked before signaling. Fully detached descendants without bridge receipts remain outside the cleanup claim. Backend error records also exclude raw private request/error text.

The [new retained record](evidence/client-qualification-remediation-2026-10-03.json) completed 12 recorded runs, with interruption, native-session resume and bridge restart passing for the same four installed client versions. No forced cleanup was needed, all 12 observed bridges exited and all disposable profiles were removed. The default-policy denial rows above remain unchanged. Pi exited after 30.391 seconds, consistent with the fixture's 30-second I/O timeout; this does not establish immediate cancellation. The record identifies the protocol-1 working source and rebuilt probe hashes at the time of the run. All 76 Python tests passed, including subprocess cleanup and redaction regressions. These results remain scripted-provider fixture evidence, with the recorded policy opt-ins and network guard, rather than production Locust lifecycle or real-model qualification.

## Droid failure and controlled correction

The [initial committed-harness run](evidence/client-qualification-2026-10-03.json) passed Droid initialization, read/write, held wait and interruption, but resume exited 1 with empty stdout/stderr. An independent reviewer reproduced the same failure after an uninterrupted clean turn, excluding SIGINT as its sole cause. An isolated Factory diagnostic log identified `Failed to fetch session`; local inspection of the installed executable's readable bundled code showed a Factory session lookup before loading local history. The [diagnosis/control record](evidence/droid-resume-diagnosis-2026-10-03.json) retains the executable hash, method and observed outcomes without copied bundled source.

The corrected fixture explicitly sets `FACTORY_API_BASE_URL` and `FACTORY_API_BASE_URL_EU` to loopback and returns HTTP 404 to `GET /api/sessions/<id>`. This exercises Droid's local-session fallback. The lookup is recorded separately from model requests with its identifier redacted. The full held-wait/SIGINT/resume sequence then passed, with the same native ID, a fresh bridge and a new authenticated read. This establishes the local fallback under a scripted Factory backend; it does not qualify Factory account authentication, cloud session retrieval or arbitrary service failures. The initial failure is preserved as an experiment limitation, not described as a universal Droid defect.

The task/review flow, protected real-daemon handshake, live models/accounts, hooks/delivery, three machines and packaged install remain unverified.
