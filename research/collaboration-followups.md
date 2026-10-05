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
8. **A terminal attempt report released the claim before patch submission.**
   In the first Claude-coordinator/Pi-worker simulation, Pi edited, tested,
   captured and inspected its patch, then reported the attempt completed before
   submitting. Submission was superseded, another report conflicted, and the
   pinned organization rules refused starting another attempt under the same
   offer. The model had published a goal-wide finding, but stopped without a
   task-backed contribution. The skill now explicitly orders task-backed
   publication before a terminal report. The failed run is preserved separately
   from the retry; the rules were not weakened to admit the late submission.
9. **A mistyped working directory prevented an actual peer review.** Codex/Merak
   attempt 02 reached a revised artifact that passed all fourteen private cases,
   with the later finding read, acknowledged and declared in signed sources.
   Merak's reviewer supplied a macOS temporary directory missing one character,
   repeated the failing command three times, then finished without approval.
   Application was correctly withheld. Later fixture guidance uses the relative
   `./locust-scoped` command and the runner's existing working directory; private
   runtime profiles now use a short owned directory under `/tmp`.
10. **The fixture omitted an explicit Python version.** The initial builder
    tried an unavailable `python` command and then encountered annotation syntax
    behavior under its resolved `python3`. It recovered during the run. A separate
    reproduction with the same private environment and `/bin/zsh -lc` resolved
    `/usr/local/bin/python3` as Python 3.9.7; the agent itself did not record a
    version probe. Future
    acceptance fixtures declare Python 3.12 or newer and provide the harness's
    exact interpreter path. This changes fixture environment guidance, not the
    private behavioral constraint or Locust permissions.
11. **Codex resume usage was cumulative.** Independent inspection of retained
    native token receipts established that the CLI's `turn.completed.usage`
    carries totals across the resumed thread. For attempt 02 the input endpoints
    were 69,477, 1,223,987 and 2,733,876; the three phase deltas are 69,477,
    1,154,510 and 1,509,889. Adding the endpoints would overcount. Per-phase
    accounting now subtracts the preceding receipt from the same native thread,
    retains the raw endpoints, and keeps missing provider billing explicit.
    Claude's resumed result has mixed semantics: `usage` is per invocation,
    while `modelUsage` and reported list cost are cumulative. In its retry the
    review cost endpoint was $0.1086813 and the resumed endpoint $0.2032934;
    application added $0.0946121. Pi's new `message_end` receipts account for
    that invocation. The evidence keeps these client-specific meanings rather
    than adding similarly named fields indiscriminately.
12. **A failed initial oracle alone was insufficient evidence of revision.**
    The first scenario predicate would also accept an unchanged starter or
    invalid initial program if a later artifact passed. Review tightened it to
    require the initial implementation pass the original portability oracle,
    have an independently reconstructed signed patch, and fail the later
    deployment-specific oracle. Regression mutations cover those false passes.
13. **A substring did not prove the peer inspected the exact patch.** The first
    review predicate matched a target ID anywhere in successful command output.
    A mutation that echoed that ID but reviewed another 64-character patch ID
    still passed. The corrected predicate parses the returned review object and
    binds its patch, base, head and diff to the independently inspected artifact.
14. **Successful work did not mean every requested checkpoint happened.** Claude
    omitted the compact freshness checkpoint in the first successful worker turn
    and in its later coordinator review. Full context reads and acknowledgments
    succeeded. These are retained instruction-adherence gaps; passing the work
    scenario does not turn those missing reads into successful observations.
15. **Local review permission did not make an open organization peer-reviewed.**
    Codex/Merak attempt 03 passed the initial and revised artifact checks, exact
    finding acknowledgment and signed attribution. Merak inspected the exact
    patch, but its signed review was refused: the fixture had chosen the `open`
    preset, whose completion policy uses author declaration. The review grant
    did not override candidate eligibility, and pending work correctly reported
    no review obligations. Merak published its assessment and blocker; the
    harness withheld application. This was a fixture policy error, not a product
    authorization failure. The next scenario uses the existing `peer-review`
    preset, whose policy explicitly admits review by another member.
16. **A partial transcript can still contain a complete review.** Merak's
    diagnostic export marked the overall transcript partial, while the relevant
    completed patch-review effect contained its full, nontruncated JSON result.
    The page reported 1,048,038 output bytes, `max_output_bytes` truncation and
    continuation at effect 58 of 62; its exporter has a 1 MiB page ceiling.
    The original harness rejected that review solely because of the top-level
    flag. The corrected predicate checks completeness of the relevant tool
    result and equality with the independently authenticated patch, base, head
    and diff. A truncated review body or a different patch still fails. Overall
    transcript incompleteness remains visible; later assessment of an existing
    run is labeled separately from the predicate used when it ran.

## Live collaboration and other clients

The live scenarios retain each attempt, exact binary identities,
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

The [Pi/Claude evidence](evidence/collaboration-live-pi-claude-2026-10-04.json)
records a successful Pi-coordinator/Claude-worker workflow, the failed reverse
attempt described above, and a successful reverse retry after the skill change.
Both successful directions independently verified the exact repaired file, all
five unchanged tests, patch selection before application, native coordinator
session continuation, application, and preservation of unrelated work and Git
HEAD. All seven native turns cleaned up naturally and their private profiles
were removed. The clients used GPT-6 Luna and Claude Haiku 4.5 with observed
native model IDs. They shared one enrolled principal with separate protected
sessions; this is not independent-principal or physical-network evidence.

The successful original Pi/Claude case reports $0.13721318 in combined native
list/catalog estimates; the failed Pi worker reports $0.01444448; the successful
reverse retry reports $0.2156693 after correcting cumulative Claude accounting.
These are estimates reported by the clients, not verified provider charges.
No live context page in this small fixture had a continuation cursor; live
multi-page completeness is not claimed. The frozen `runtime02` executable
predates the final invitation-refusal fix and was kept unchanged throughout
the single-daemon scenarios.

The [Codex/Merak evidence](evidence/collaboration-live-codex-merak-2026-10-04.json)
retains four attempts. Attempt 01 stopped at the permission-inspection defect;
attempt 02 reached revision but failed review because of the wrong working
directory; attempt 03 reached exact inspection but the fixture policy refused
peer review. Attempt 04 passed all ten original assertions. Its Codex builder
continued the same native thread and Locust session after the person's grant,
published a valid initial patch, then read and acknowledged the later Merak
finding and revised its implementation with that source in the signed metadata.
Merak inspected and approved the exact final contribution. The person harness
applied it in a separate Git workspace; all fourteen behavior cases passed and
the dirty tracked README, untracked sentinel and Git HEAD were preserved. A
separate root recheck of the retained evidence and applied artifact passed too.
Both agents used GPT-6 Luna; the roles had separate enrolled principals.

Attempt 04's five model invocations used these corrected phase counters:

| Phase | Input tokens | Cached input | Uncached input | Output tokens | Client-reported dollars |
| --- | ---: | ---: | ---: | ---: | ---: |
| Codex permission stop | 68,892 | 50,484 | 18,408 | 722 | unavailable |
| Codex initial implementation | 963,034 | 927,849 | 35,185 | 12,130 | unavailable |
| Merak research | 243,359 | 214,283 | 29,076 | 5,091 | $0.008328 |
| Codex revision | 1,461,762 | 1,426,259 | 35,503 | 9,973 | unavailable |
| Merak review | 614,606 | 574,628 | 39,978 | 7,222 | $0.014366 |

That successful attempt totals 3,351,653 input tokens: 3,193,503 cached and
158,150 uncached, plus 35,138 output tokens. Across all four attempts, sixteen
paid invocations total 9,625,471 input tokens: 9,140,625 cached and 484,846
uncached, plus 97,184 output tokens. The six Merak invocations return $0.061605
combined in client cost fields without a cost-source field; the ten Codex
invocations return no dollar amount. A complete campaign dollar total and
provider-billed charges are therefore unavailable. These scenarios differ from
the earlier pilot, so their costs do not establish a paired optimization win.

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

After the live harness corrections, the complete Python suite passes all 230
tests. Focused mutations reject an unchanged or invalid initial implementation,
wrong signed patch reconstruction, mismatched acknowledgment/source evidence,
wrong peer patch reads, missing application/work preservation and invalid usage
counter deltas. Independent review confirmed the three corrected evidence gaps.
Polaris source commit `4917caf3963900c5ebaa6819eaa034663e589d64` pins Locust
`c50a43f54a6168a631351c6500b44ccd1167d27c`; its current native SDK/component and
frontend checks are recorded in the [Polaris guide](https://github.com/andsav/locust.farm/blob/ddb2db1e609652a1de766b453d8e86b45b25a1f3/docs/guide/polaris.md).
