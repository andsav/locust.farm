# Collaboration follow-ups and simulation findings

Updated 2026-10-04. This records work after the
[performance pass](performance-cost-pass.md), including failed attempts and the
limits of each observation. The owner has tested physical-machine behavior and
explicitly excluded repeating that acceptance work. Droid is a lower priority
than Codex, Merak, Pi and Claude. This is an implementation and investigation
record, not an additional release checklist.

## Implemented runtime changes

- **Short acknowledgment references.** Named CLI commands and MCP return a
  68-character `ctx:` reference instead of requiring the model to copy the full
  signed receipt. A private client cache binds it to credential and execution
  session. Atomic writes, directory/file ownership and permissions, no-follow
  opens, content hashes and the daemon's original signature checks protect the
  mapping. The native API and raw `locust call` retain the typed signed object.
  Reads and lost responses never acknowledge content. CLI/MCP references work
  across client restarts; another session cannot resolve or acknowledge them.
- **Explicit context size.** `context.read` requires `view=full` or `compact`.
  Only the first page repeats scope metadata. Compact summaries contain all
  pending category counts; `pending.page` retrieves detailed obligations with
  an explicit limit, optional category and revision-bound continuation. Event
  text is identical between views unless the caller requests a preview. At
  256 already-read findings and 192 review obligations, the native JSON response
  is 745 bytes compact versus 103,113 bytes full. This is a size observation,
  not a latency or billed-cost estimate.
- **Signed source declarations.** Contributions carry source event IDs;
  `patch submit --source EVENT` is repeatable. `contribution.inspect` returns
  the source authors, current standing and available text, plus the exact
  attempt/task-round chain. Absent headers and unavailable text stay explicit.
  Declared sources do not become authority, approval evidence or causal
  dependencies. They are not proof of comprehension or actual use.
- **Own permission inspection.** Agents and their viewers can inspect their own
  standing permissions and task authorizations through CLI/MCP. Inspecting
  another principal or changing any permission still requires the owner.

The signed contribution layout and local response layout change the current
format to API 4, protocol 4 and store schema 4. Existing incompatible homes and
peers are refused; there is no migration or legacy decoder. See the
[runtime reference](../docs/guide/runtime-reference.md) for the exact surfaces.

## Findings from actual processes and transport

1. **Already-received invitation refusals could disappear.** The full workspace
   run reproduced the earlier timeout specifically while a revoked invitation
   remained `Joining`. The initiator had decoded an authenticated refusal, but
   the responder closed before acknowledgment of the initiator's own FIN. The
   driver's `Closed` branch replaced that known refusal with `Aborted`. A
   deterministic driver regression failed before the fix. The driver now
   preserves an already-decoded remote refusal; successful completion and
   locally emitted refusals still require their existing finish confirmation.
   Three runs of the original real-daemon invitation scenario then passed with
   the unchanged 20-second per-phase watchdog. This reproduces the state-loss
   mechanism; the original historical run had insufficient diagnostics to prove
   its exact event ordering retrospectively.
2. **An unadmitted connection could close under a pending stream.** A separate
   network scheduling race counted established exchanges but omitted a reserved
   outbound opening. Its real-QUIC regression fails with the old close predicate
   and passes after tracking pending openings. This fix is independent of the
   refusal-state loss above.
3. **A live Codex agent was denied its own permission inventory.** In paid
   attempt 01 the agent correctly observed execution disabled, tried to inspect
   its own permissions, and stopped to ask the person for permission. The
   operation had been owner-only. Self-inspection is now allowed and model
   exposed; changing grants remains owner-only. The first simulation also
   incorrectly required the model to try an unauthorized action. Its evidence
   rule now accepts a verified missing-permission observation and a safe stop,
   then requires a real person grant and the same agent/session's successful
   continuation. Attempt 01 remains retained, not replaced by the rerun.
4. **New tagged response variants failed on the native wire.** Pure context
   tests passed, but actual CLI-to-daemon reads could not decode adjacent-tagged
   Serde enums through postcard. Externally tagged variants now match the
   existing binary codec, and both context modes plus all seven pending item
   categories are tested through native encode/decode.
5. **Named CLI commands ignored optional schema defaults.** Adding optional
   `sources: []` exposed this: the command builder supplied null instead of the
   schema's default array. Three executable manual recipes and two production
   Python cases failed. The parser now honors declared defaults; the recipes
   and affected persistent-daemon cases pass again.
6. **Multicast failure is below the goal protocol.** A standalone UDP probe on
   this host could not send IPv4 multicast through either the default route or
   explicitly selected active interface (`errno 65`, no route to host). Explicit
   loopback multicast succeeded and self-delivered. No OS settings were changed.
   This narrows the local-only discovery failure to the active-interface
   multicast path in the current process environment; it does not identify an
   OS permission or firewall cause, nor contradict the owner's machine testing.
7. **Droid's child failure depends on the qualification sandbox.** A fresh
   scripted-provider probe without Locust, MCP, driver or process cleanup still
   saw native Execute children die under `sandbox-exec`. Even an allow-default
   sandbox reproduced it. The same native client and echo command succeeded
   without that wrapper; a directly launched command also succeeded under the
   original guard. Thus the observation is a Droid × inherited macOS sandbox
   interaction on this host, not a universal Droid or Locust failure. Further
   Droid work was stopped at the owner's request.

## Live collaboration and other clients

The ongoing live scenarios retain each attempt, exact binary identities,
requested/observed models where available, prompts, tool effects, token usage,
artifact checks and cleanup results. Client token accounting, estimated client
cost and provider-billed charges are separate fields; unavailable billing is not
reported as zero. Successful tool calls, declared citations and acknowledgments
are distinguished from independent artifact checks and actual application.

The strengthened case supplies a deployment-specific reserved archive directory
only to the researcher, after the builder's initial implementation. It exercises
safe permission blocking and continuation, later evidence, revision with signed
sources, peer review, and application to a separate Git workspace with unrelated
tracked and untracked work. Pi and Claude exercise their native client paths.
Results and failure details will be appended after those runs finish.

## Runtime verification checkpoint

The [runtime evidence](evidence/collaboration-runtime-2026-10-04.json) identifies
the source fingerprints and retained check logs. The completed runtime changes pass all 689 Rust tests (14 explicit ignores),
strict workspace Clippy, formatting, six generated blueprint examples, and all
four existing executable documentation recipes. Native context/pending codec
round trips, private receipt storage, cross-client acknowledgment, own-permission
reads, signed source inspection and both transport races have targeted coverage.
The recorded failures above remain part of the investigation; the full workspace
was rerun successfully after their fixes. Live-model and other native-client
runs are recorded separately, with their exact frozen candidate bytes.
