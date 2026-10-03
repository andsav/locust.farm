# hcom validation evidence

Date: 2026-10-03. **Status: actual local component, CLI and scripted-provider real-client test results, not Locust qualification or a live-model/WAN test.** Interpret with the [hcom dissection](../hcom-dissection.md).

## Source and environment

- Source: [aannoo/hcom commit 132250bca22d7f25ac6e1fe3ac04710a6bfd5283](https://github.com/aannoo/hcom/commit/132250bca22d7f25ac6e1fe3ac04710a6bfd5283), package 0.7.27, exact upstream Cargo.lock.
- Host: macOS arm64. Compiler selected explicitly: Rust 1.96.1, matching Locust's pinned available toolchain. Upstream rust-toolchain.toml specifies 1.97.1 and Cargo.toml declares MSRV 1.88. This is not an upstream pinned-compiler qualification.
- Read-only source clone: `output/hcom-source`. Tests used a separate temporary local clone outside the Locust workspace, with no upstream application changes for the two existing suites.
- The first build attempt inside `output/hcom-source` stopped before compilation because Cargo found the surrounding Locust workspace. Moving the exact source to a separate temporary checkout resolved this setup issue; no manifest/workspace edit was made.
- Existing integration fixtures use fresh temporary homes/state/client configuration and cleared environments. Specific ignored Codex/Claude tests were selected after inspecting their fixtures; results are recorded below. No live-model provider or live broker was used. Pinned client binaries were installed under the disposable checkout only; the user's client profiles were not reconfigured. Probe mutation was limited to adding the original test below in a second disposable checkout.

## Existing suites

Command, from the separate pinned hcom checkout:

```sh
cargo +1.96.1 test --locked --test send_delivery --test cli_smoke -- --test-threads=1
```

Exit status: **0**. Results: **40 CLI smoke cases and 23 send/delivery cases passed**. Some support-module unit cases appear in both test binaries; 63 is the number of passing test executions, not a claim of 63 independent end-to-end scenarios. These tests exercise local hcom CLI/storage and delivery contracts without a real model. They do not establish current Codex/Claude/Pi behavior or a Locust adapter.

Captured test-run output follows; dependency download/compile progress preceding it is omitted.

```text
     Running tests/cli_smoke.rs (target/debug/deps/cli_smoke-e1affe1e2f5de26d)

running 40 tests
test ai_tool_broadcast_to_many_requires_go_preview ... ok
test antigravity_e2e_hook_dispatch ... ok
test bigboss_send_bypasses_identity_gate ... ok
test commands_on_stopped_agent_explain_when_and_how_to_resume ... ok
test config_known_unset_key_reports_not_set ... ok
test config_unknown_key_is_rejected ... ok
test copilot_e2e_hook_dispatch ... ok
test cursor_e2e_hook_dispatch ... ok
test events_empty_in_fresh_dir ... ok
test events_wait_cli_arriving_unread_then_matching_status_exits_zero ... ok
test events_wait_cli_preexisting_unread_times_out_with_one ... ok
test fixture_drop_terminates_orphan_without_instance_row ... Sent SIGTERM to orphan process 17021 (orphan)
Killed 1
ok
test fixture_drop_terminates_registered_process_group ... No processes with tracked PIDs found
ok
test from_in_message_text_does_not_suppress_the_name_drift_warning ... ok
test help_prints_and_exits_zero ... ok
test intent_and_reply_to_roundtrip ... ok
test lifecycle_events_emitted_for_start_and_stop ... ok
test list_json_empty ... ok
test non_destructive_reset_paths_deliver_pending_messages ... ok
test pi_e2e_hook_dispatch ... ok
test remote_reply_to_origin_resolves_imported_event_row ... ok
test send_is_quiet_when_name_matches_the_shell_identity ... ok
test send_rejects_ambiguous_bare_word_and_self_target ... ok
test send_strips_redundant_trailing_name_from_auto_resolved_sender ... ok
test send_to_missing_agent_lists_available ... ok
test send_warns_when_name_disagrees_with_the_shell_identity ... ok
test send_with_attached_from_skips_the_name_drift_warning ... ok
test send_with_from_skips_the_name_drift_warning ... ok
test send_without_identity_errors_with_hint ... ok
test start_as_reclaims_stopped_identity ... ok
test start_send_events_roundtrip ... ok
test status_clean_logs_displays_path ... ok
test status_json_in_fresh_dir ... ok
test support::claude_mock::startup_answers_each_gate_once ... ok
test support::claude_mock::startup_gate_treats_overlapping_trust_and_continue_text_as_one_gate ... ok
test support::claude_mock::trust_accept_selected_follows_the_cursor ... ok
test support::launch_scripts_dump_prints_generated_scripts_but_not_the_env_sidecar ... ok
test unknown_agent_gets_typo_suggestion ... ok
test unknown_command_and_tool_suggest_corrections ... ok
test unknown_command_errors ... ok

test result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.76s

     Running tests/send_delivery.rs (target/debug/deps/send_delivery-093b589cf5810c0a)

running 23 tests
test automatic_delivery_tools_preserve_inbox_on_send ... ok
test command_drain_delivers_capped_prefix ... ok
test contiguous_prefix_adhoc ... ok
test early_filter_listen_match_still_delivers ... ok
test external_sender_delivers_to_process_bound_instance ... ok
test external_sender_preserves_inline_receive_delivery ... ok
test failed_command_delivery_write_fails_command ... ok
test failed_send_still_delivers_pending_messages ... ok
test failed_stdout_write_preserves_incoming_messages ... ok
test filter_listen_reads_past_capped_batch ... ok
test filter_listen_shows_system_messages ... ok
test late_send_cannot_rewind_a_newer_cursor ... ok
test listen_delivers_capped_prefix ... ok
test listen_exiting_before_reading_still_delivers ... ok
test message_arriving_mid_listen_is_noticed ... [Listening for messages to nori. Timeout: 0.1s]
ok
test message_arriving_mid_send_is_noticed ... ok
test mixed_main_and_child_messages_share_one_batch_limit ... ok
test quiet_send_preserves_incoming_messages ... ok
test relay_reply_ids_survive_inline_receive ... ok
test support::claude_mock::startup_answers_each_gate_once ... ok
test support::claude_mock::startup_gate_treats_overlapping_trust_and_continue_text_as_one_gate ... ok
test support::claude_mock::trust_accept_selected_follows_the_cursor ... ok
test support::launch_scripts_dump_prints_generated_scripts_but_not_the_env_sidecar ... ok

test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.97s

```

## Real Codex with a scripted localhost provider

Pinned client: **Codex 0.159.2**, installed only under the temporary checkout's `target/mock-tools`. The upstream fixture launches the actual CLI and native hooks/PTY against its local Responses API mock, with dummy credentials and isolated state. No live-model call is needed.

Setup and test commands, from that checkout:

```sh
npm install --global --prefix target/mock-tools --cache target/npm-cache --no-audit --no-fund @openai/codex@0.159.2 '@openai/codex-darwin-arm64@npm:@openai/codex@0.159.2-darwin-arm64'
PATH="$PWD/target/mock-tools/bin:$PATH" cargo +1.96.1 test --locked --test real_tool_codex -- --ignored --nocapture --test-threads=1
```

The executed test also set `HCOM_TEST_KEEP_DIR` to a task-owned output directory so any failures could be inspected. Both selected cases passed: **lifecycle launch/send/fork/kill/resume/cleanup**, and **approval gate holds pending delivery then releases it after approval**. This is executable evidence of hcom's native-client integration on this macOS host.

It does not qualify a Locust backend, its bootstrap/policy configuration, or actual model cooperation. The lifecycle case uses `--yolo`; the separate approval case explicitly configures workspace-write and on-request. hcom's own launch transformations still apply. Do not present the combined pass as proof of untouched default client policy or whole-process sandboxing.

Captured output:

```text
   Compiling hcom v0.7.27 (/private/var/folders/22/61v9z9kd34x959r1k7tbdw480000gn/T/locust-hcom-review-qma753sg/hcom)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.83s
     Running tests/real_tool_codex.rs (target/debug/deps/real_tool_codex-b068bf848c0d2948)

running 2 tests
test real_codex_approval_gate_blocks_pending_message_then_clears_on_approval ... Sent SIGTERM to 'limo'
  To resume: hcom r limo
Killed 1
ok
test real_codex_full_lifecycle_send_fork_kill_resume_and_cleanup ... No processes with tracked PIDs found
ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out; finished in 13.98s

```

## Real Claude with a scripted localhost provider

Pinned client: **Claude Code 2.1.283**, installed only under the temporary checkout's `target/mock-tools`. The fixture uses dummy authentication and a local Anthropic Messages mock, with isolated profiles. The initial two-case suite exited **101**: full lifecycle passed, approval-resume failed. The approval case alone then passed unchanged on one repeat. Preserve the initial failure; this is an intermittent qualification result, not a green two-case run.

Setup and commands, from the separate pinned hcom checkout:

```sh
npm install --global --prefix target/mock-tools --cache target/npm-cache --no-audit --no-fund --ignore-scripts @anthropic-ai/claude-code@2.1.283 @anthropic-ai/claude-code-darwin-arm64@2.1.283
node target/mock-tools/lib/node_modules/@anthropic-ai/claude-code/install.cjs
PATH="$PWD/target/mock-tools/bin:$PATH" cargo +1.96.1 test --locked --test real_tool_claude -- --ignored --nocapture --test-threads=1
PATH="$PWD/target/mock-tools/bin:$PATH" cargo +1.96.1 test --locked --test real_tool_claude real_claude_approval_gate_blocks_pending_message_then_clears_on_approval -- --exact --ignored --nocapture --test-threads=1
```

The postinstall script was inspected first: it places the selected native binary inside that local npm prefix. Test runs additionally set `HCOM_TEST_KEEP_DIR` to separate task-owned output directories. The lifecycle case uses `dontAsk` with selected allowed tools. The approval case explicitly requests `manual` mode; its comments referring to default permission mode do not override the actual arguments.

Failure boundary: hcom reported the client blocked on approval with one unread inbound message. The test sent a terminal approval keystroke successfully, then timed out waiting for the gated command's exact output. The captured terminal still showed the permission question. This establishes a failed approval-resume observation, not a permission bypass. Whether the fault is in test timing, terminal injection or client handling was not isolated. One unchanged repeat passed; no code fix was applied.

The following diagnostic excerpts retain the failure header, session listing, and final test summary; trailing whitespace is removed. The full temporary log also contained repetitive terminal escape output and an unrelated process inventory; those are not reproduced here.

```text
running 2 tests
test real_claude_approval_gate_blocks_pending_message_then_clears_on_approval ...
thread 'real_claude_approval_gate_blocks_pending_message_then_clears_on_approval' (91695483) panicked at tests/support/mod.rs:802:17:
timed out waiting for approved command executed with the exact gated content
last poll error: <none>

--- list --json (exit 0) ---
[{"agent_id":null,"background_log_file":null,"base_name":"loki","created_at":1791065050.0,"description":"tool:send","directory":"/private/var/folders/22/61v9z9kd34x959r1k7tbdw480000gn/T/locust-hcom-review-qma753sg/hcom","headless":false,"hooks_bound":false,"launch_context":{},"name":"loki","parent_name":null,"process_bound":true,"session_id":"","status":"inactive","status_age_seconds":40,"status_context":"tool:send","status_detail":"","tag":null,"tool":"adhoc","transcript_path":null,"unread_count":0},{"agent_id":null,"background_log_file":"/Users/andrei/Projects26/locust/output/hcom-real-claude/.tmp7rtAaS/hcom-state/.tmp/logs/background_1791065050_3384.log","base_name":"mina","created_at":1791065050.0,"description":"blocked: approval pending","directory":"/Users/andrei/Projects26/locust/output/hcom-real-claude/.tmp7rtAaS/workspace","headless":true,"hooks_bound":true,"launch_context":{"env":{},"git_branch":"main","pid_identity":"apple:1791065050:899098","process_id":"8ba95dff-56c1-4629-94a5-4fafef51c51d","terminal_preset_effective":"default","tty":""},"name":"mina","parent_name":null,"process_bound":true,"session_id":"2839ee1c-b795-4405-9827-daca9fec0b9f","status":"blocked","status_age_seconds":40,"status_context":"approval","status_detail":"node -e \"require('fs').appendFileSync('/Users/andrei/Projects26/locust/output/hcom-real-claude/.tmp7rtAaS/workspace/approval-result.txt', 'HCOM_CLAUDE_GATED_1791065049862872000\\\\n')\"","tag":null,"tool":"claude","transcript_path":"/Users/andrei/Projects26/locust/output/hcom-real-claude/.tmp7rtAaS/claude-home/projects/-Users-andrei-Projects26-locust-output-hcom-real-claude--tmp7rtAaS-workspace/2839ee1c-b795-4405-9827-daca9fec0b9f.jsonl","unread_count":1}]

preserving failed real-tool test directory: /Users/andrei/Projects26/locust/output/hcom-real-claude/.tmp7rtAaS
Sent SIGTERM to 'mina'
  To resume: hcom r mina
Killed 1
FAILED
test real_claude_full_lifecycle_send_fork_kill_resume_and_cleanup ... No processes with tracked PIDs found
ok

failures:

failures:
    real_claude_approval_gate_blocks_pending_message_then_clears_on_approval

test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 5 filtered out; finished in 59.05s

error: test failed, to rerun pass `--test real_tool_claude`
```

Exact repeat output:

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.20s
     Running tests/real_tool_claude.rs (target/debug/deps/real_tool_claude-824b14a1772ae638)

running 1 test
test real_claude_approval_gate_blocks_pending_message_then_clears_on_approval ... Sent SIGTERM to 'midi'
  To resume: hcom r midi
Killed 1
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 9.78s

```

Required before claiming a Locust hcom backend: reproduce and resolve the intermittent approval-resume path under its intended local policy. The passing lifecycle evidence still supports evaluating substantial reuse of hcom's launch/hook/PTY/session machinery.

## Original storage-error characterization

Question: if storing a received event fails, does hcom leave it recoverable by a normal retry?

Method: start at remote event cursor 100; install a temporary SQLite trigger rejecting message insertion; import event 101 through the actual private `import_remote_events`; inspect rows, cursor and gap state; remove the trigger; retry identical event 101. This reproduces a statement failure rather than a literal disk-full condition. No network transport is involved.

Observed: the first import reports progress, retains zero rows, advances to 101 and records no missing range. After the trigger is removed, the same event is skipped as already imported. This demonstrates loss on this receive/retry path. It does not show that every independent repair mechanism must fail.

Reproduction: clone the pinned revision into a directory outside a containing Cargo workspace, append the following original test module to `src/relay/pull.rs`, then run the command below. Do not change the production functions. The module uses hcom's existing isolated test helper.

```rust

#[cfg(test)]
mod locust_characterization_tests {
    use super::*;
    use crate::hooks::test_helpers::isolated_test_env;
    use serde_json::json;

    #[test]
    fn locust_import_insert_failure_advances_cursor_and_prevents_retry() {
        let (_dir, _hcom_dir, _home, _guard) = isolated_test_env();
        let db = HcomDb::open().unwrap();
        let device = "locust-probe-device";
        let cursor_key = format!("relay_events_{device}");
        safe_kv_set(&db, &cursor_key, Some("100"));
        let events = vec![json!({
            "id": 101,
            "ts": "2026-10-03T12:00:00.000000+00:00",
            "type": "message",
            "instance": "luna",
            "data": {"from": "luna", "text": "must survive failed insert", "mentions": ["nova"]}
        })];
        db.conn().execute_batch(
            "CREATE TEMP TRIGGER fail_probe_message BEFORE INSERT ON events
             WHEN NEW.type = 'message'
             BEGIN SELECT RAISE(FAIL, 'locust injected event insert failure'); END;"
        ).unwrap();
        let imported = import_remote_events(&db, device, "ABCD", &events, 0.0, "MINE");
        let rows: i64 = db.conn().query_row(
            "SELECT COUNT(*) FROM events WHERE json_extract(data, '$._relay.device') = ?",
            rusqlite::params![device], |row| row.get(0)
        ).unwrap();
        let cursor = safe_kv_get(&db, &cursor_key);
        let gaps = crate::relay::backfill::load_gaps(&db, device);
        println!("after_failed_insert: reported_imported={imported}, cursor={cursor:?}, retained_rows={rows}, recorded_gaps={}", gaps.len());
        assert!(imported, "characterization: current code reports new imported events");
        assert_eq!(cursor.as_deref(), Some("101"));
        assert_eq!(rows, 0);
        assert!(gaps.is_empty());

        db.conn().execute_batch("DROP TRIGGER fail_probe_message").unwrap();
        let retried = import_remote_events(&db, device, "ABCD", &events, 0.0, "MINE");
        let retry_rows: i64 = db.conn().query_row(
            "SELECT COUNT(*) FROM events WHERE json_extract(data, '$._relay.device') = ?",
            rusqlite::params![device], |row| row.get(0)
        ).unwrap();
        println!("after_storage_recovers_and_same_event_retries: reported_imported={retried}, cursor={:?}, retained_rows={retry_rows}", safe_kv_get(&db, &cursor_key));
        assert!(!retried, "characterization: retry is suppressed by advanced cursor");
        assert_eq!(retry_rows, 0);
    }
}
```

```sh
cargo +1.96.1 test --locked --bin hcom relay::pull::locust_characterization_tests::locust_import_insert_failure_advances_cursor_and_prevents_retry -- --exact --nocapture
```

The executed probe used `CARGO_TARGET_DIR` pointing at the earlier temporary checkout's target directory to reuse compiled dependencies. This changes build-output placement, not source or test behavior. Exit status: **0**, because the characterization deliberately asserts the bug. A fixed implementation should fail these current-behavior assertions and pass an inverted recovery regression instead.

Captured output:

```text
   Compiling hcom v0.7.27 (/private/tmp/locust-hcom-cursor-y0es4vyb/hcom)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 11.18s
     Running unittests src/main.rs (/var/folders/22/61v9z9kd34x959r1k7tbdw480000gn/T/locust-hcom-review-qma753sg/hcom/target/debug/deps/hcom-c2d1fc16c05bfb26)

running 1 test
after_failed_insert: reported_imported=true, cursor=Some("101"), retained_rows=0, recorded_gaps=0
after_storage_recovers_and_same_event_retries: reported_imported=false, cursor=Some("101"), retained_rows=0
test relay::pull::locust_characterization_tests::locust_import_insert_failure_advances_cursor_and_prevents_retry ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2392 filtered out; finished in 0.01s

```

Relevant source: [unchecked insert followed by cursor advance](https://github.com/aannoo/hcom/blob/132250bca22d7f25ac6e1fe3ac04710a6bfd5283/src/relay/pull.rs#L679-L702), [discarded insert result](https://github.com/aannoo/hcom/blob/132250bca22d7f25ac6e1fe3ac04710a6bfd5283/src/relay/pull.rs#L783-L790).

Locust regression to implement: failed event persistence must leave the checkpoint unchanged; retry after recovery must store and project the event once. Event/projection/checkpoint durability is an API contract, independent of transport enqueue or notification delivery.
