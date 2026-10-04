# Goals and organizations

**Status: implemented development model, API 3 / protocol 3.** Local engine,
replay and daemon tests exercise this model. Public release and native-client
qualification remain separate; see [availability](status.md).

## Goals

A goal states the intended outcome and provides a shared collaboration context.
Joining a goal, accepting an assignment and finishing an artifact are distinct
acts. A participant's local execution is not the same thing as a distributed
assignment or an accepted result.

## Organizations and blueprints

An organization defines how participants coordinate: roles, assignments,
communication rules and decision authority. A blueprint is a declarative definition
of those rules. A blueprint instance adds the concrete bindings required to run
it, such as who fills a role and which authority may finalize a decision.

A valid definition can describe an organization without being ready to execute.
Offline checks cannot observe whether a participant is online, local permissions
exist, or an instance has the required authority. See [Blueprint authoring](blueprint-authoring.md).

## Participants and local agents

Distributed participant identity belongs to Locust. An agent harness performs
local work for that participant under its own tools and approval policies. Merak
is a possible local executor, not a requirement or a synthetic representation of
remote participants.

Locust does not sandbox all the tools of a participant's harness. Access to a
local shell, repository, provider account or credential remains subject to that
harness and the person's machine policy. Organization rules do not silently grant
filesystem access or weaken an existing approval boundary.

## Daemon-driven progress

The accepted design lets the daemon drive explicitly configured transitions and
durably deliver ready work. An agent does not have to ask for each transition.
Only declared transitions are eligible: a daemon must not invent organizational
rules from a diagram or an inferred intention. Delivery and local execution remain
separate observations. The sender retains an outbox entry until the receiving
daemon confirms durable inbox storage. Lost receipts retry the same logical
identity. Explicit agent acknowledgment and actual local execution remain
separate. Encoded peer-exchange tests cover lost receipts, both-daemon restart
and authority retraction; they do not prove physical-machine connectivity.

## Decisions and authority

Producing work, reviewing it and accepting it are separate responsibilities.
Rules need an explicit resolution path when authorities are missing, unavailable
or ambiguous. Offline definition checks describe declared rules; action readiness
must also consider the current instance and actor.

Unsupported definitions should receive a clear diagnostic. The greenfield design
supports one current contract; it does not offer format conversion, mixed-version
runtime branches or an old-reader fallback.

Read the [accepted organization decision](../organization-blueprints.md) for the
full model and [implementation plan](../organization-blueprints-implementation-plan.md)
for its fixed scope and [implementation ledger](../organization-blueprints-status.md)
for completed checks. Return to [Locust overview](overview.md) for
present availability.

## Context, artifacts and attempts

An artifact is an exact immutable object with a content identity and provenance.
Context inputs name the material an organization/task uses; importing material
does not execute instructions or grant access. Missing object bytes are a pending
dependency, not evidence that the object does not exist.

Tasks are optional work descriptions. An attempt is a participant's occurrence of
work; a contribution is the exact submitted finding/output, optionally attached
to a task and attempt. Several attempts can coexist. A local claim generation
fences a local execution session and does not establish a distributed exclusive
reservation. Open findings need no task acceptance or universal selected head.

## Local permissions and synchronization

Membership, role eligibility and local execution/sharing grants are independent.
A rule-authorized contribution is not permission to spend account funds or mutate
another person's checkout. The native harness still enforces its own tool policy.

A replica's view identifies its observed evidence and missing dependencies. Clocks
are diagnostic; they do not select a winner or backdate membership. The same
admissible proof/context must yield the same projection despite reception order.
Later forks and removal cutoffs can change an observed verdict; expose stale,
pending and disputed states instead of presenting an irrevocable accepted head.
