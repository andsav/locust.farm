# Troubleshooting, FAQ and glossary

## Safe next actions

| Symptom | Interpretation | Next action |
| --- | --- | --- |
| No public installer | Software publication is unavailable | Report harness capabilities and stop; do not invent a download |
| Service request succeeded but API does not answer | Manager request and authenticated readiness differ | Inspect settled service status, then authenticated daemon diagnosis |
| MCP configuration exists but no tools appear | Native discovery/readiness is unverified | Inspect the client's MCP view and supported refresh; preserve policy |
| Blueprint JSON parses but validation fails | Shape or semantics are unsupported | Follow diagnostic code/phase/path and correction; inspect exact schema |
| Offline validation passes but work cannot run | Contextual bindings or local permissions may be missing | Inspect `rules effective`, `pending` and the requested action's diagnostic |
| Review arrives before binding proof | Evidence is pending verification | Fetch/retain dependency proof and reevaluate; do not count or reject early |
| Two candidates meet review threshold | Both can qualify | Use explicit selection only if a unique output is required |
| Authority is offline | Its dependent decision waits | Continue unaffected authorized work; do not invent replacement authority |
| Checkout is dirty or patch base stale | Local application is unsafe/unsupported | Preserve local work and reconcile deliberately |
| Stop requested but no observed outcome | Cancellation is unresolved | Inspect current authorized execution; report uncertain if unobserved |
| Unsupported existing state | Current-format refusal | Preserve it; select explicitly fresh supported state, no conversion |

## Frequently asked questions

**Do I need Polaris?** No. It is an optional development interface; its native package remains unqualified. The
standalone daemon and agent surfaces are the core boundary.

**Does installing grant access to my files or account?** No. Setup, local work,
sharing and joining are separate approvals. Your harness's existing tools still
follow their native policy; Locust is not a sandbox around all of them.

**Does peer review select one winner?** No. Several candidates can independently
satisfy their rule. A unique choice requires explicit scoped selection authority.

**Can an administrator be offline?** Previously authorized independent work can
continue with its required proofs. Administrator changes wait; absence cannot
mint permission or reassign authority.

**Does the daemon advance a pipeline automatically?** The daemon drives
explicitly configured transitions and durably delivers ready work. It does not
invent transitions, launch without a local grant or promise to wake any closed
remote harness. Local tests exercise restart and logical effect deduplication; actual native
launch still requires client-specific evidence.

## Glossary

- **Definition:** immutable semantics after publication; editable drafts are separate.
- **Instance:** concrete authenticated role/input bindings to a definition.
- **Goal:** collaboration context with one membership/rule administrator.
- **Scope/round:** exact work and rule context to which evidence/decisions attach.
- **Contribution:** exact artifact or finding, optionally attached to a task/attempt.
- **Attempt:** one participant's work occurrence, distinct from a global reservation.
- **Criterion satisfaction:** candidate evidence meets its pinned supported rule.
- **Selection:** explicit authority chooses an exact output when uniqueness is required.
- **Runner:** explicitly authorized signer of configured flow effects.
- **Delivery:** committed ready work reaches an inbox; not execution start.
- **Tenure:** one authenticated admission interval, distinct from later re-admission.
- **Proof closure:** retained signed dependencies needed to reproduce a verdict.

## Development release notes

The development runtime now uses API 2 / protocol 2 with organization rules,
private drafts/publication, taskless contributions, independent attempts, scoped
completion and durable flow. The site includes raw Markdown, versioned inventories,
search and generated CLI/API/MCP/event references. Public software publication and
website deployment remain unavailable. See the
[implementation ledger](../organization-blueprints-status.md) for current evidence.
