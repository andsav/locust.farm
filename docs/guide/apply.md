# Apply a chosen patch

**Status: current organization application flow is an implementation target.**
Choosing a contribution and changing a local checkout are independent actions.
The offline authoring commands do neither. This procedure defines what a runtime
apply journey must observe before it can be qualified.

## Select an exact output

Inspect the contribution identity, base snapshot, patch artifact, provenance and
applicable rule/round. Approval of one candidate does not attach to the author's
next submission. A review threshold can qualify several candidates; only an
explicit selection authority decides one output if the arrangement requires it.
If that authority is unavailable, selection waits while unrelated authorized
work can continue.

You may deliberately choose an eligible taskless output where the rule permits
it. There is no universal goal accepted head that every open finding must create.
A selected output is still not applied to your working copy.

## Review the local checkout

Identify the exact target workspace and branch. Compare its current base with the
contribution's base and inspect staged, unstaged and untracked work. Preserve
unrelated local changes. A stale base, missing artifact or dirty apply conflict
is an actionable refusal, not success or permission to force-reset the checkout.

Review the files and expected patch before granting local application. A goal
administrator or remote reviewer cannot grant arbitrary filesystem mutation on
your machine. An existing local grant covers only its declared scope.

## Apply and verify

The local executor applies the chosen exact artifact under the supported base and
ownership checks. Inspect actual file changes and run the project checks relevant
to those changes. Record the observed outcome with the contribution and checkout
identity. An `applied` bookkeeping record is not independent verification of files.

If apply fails, retain the artifact and explain the conflict. Reconcile your local
work explicitly, prepare a fresh applicable candidate or decline it. Do not
report an attempted application as merged. Accepting, applying, committing and
pushing are separate decisions; this procedure grants no push authorization.

## Recovery evidence

After an interruption, determine whether the patch actually changed the target
before retrying. A timeout or missing receipt is an uncertain outcome. Keep exact
artifact/base identity so a repeated observation cannot silently target different
bytes. [Recovery](recovery.md) distinguishes uncertainty from rejection and
[completion](completion.md) explains the separate distributed verdict.
