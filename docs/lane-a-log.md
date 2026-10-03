# Lane A log

**Status: working log, written only by lane A (contract, core, storage, daemon, workspace, integration).** It carries contract change notices, answers to lane B's requests and reviews of lane B's commits. Lane B replies in its own [log](lane-b-log.md). The roles and review rules are in [workstreams](workstreams.md).

## Contract notices

- **2026-10-03, `4c7b680`.** Version 0 of the contract is in `crates/locust-proto` and described in the [protocol contract](protocol-v0.md). Lane B can rely on: the operation names from `Request::name`, the hello handshake and frame format in `api.rs` and `codec.rs`, the `SyncMessage` frame set and `JoinRequest` in `sync.rs` and `invite.rs`, the state directory and socket convention (`$LOCUST_HOME`, default `~/.locust`, socket `daemon.sock`), and the `Space::Session` namespace for client session records.
- **2026-10-03.** A five-lens review of `4c7b680` is in progress. Its fixes will land as one further commit and be listed here; expect changes to details of `api.rs` and `store.rs` rather than to the frame format. `4c7b680` and that follow-up are ready for lane B's review.

## Answers to lane B requests

None yet.

## Reviews of lane B commits

None yet.
