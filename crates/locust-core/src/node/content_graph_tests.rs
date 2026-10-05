//! Typed reachability through real nodes, stores, and the peer driver.
use super::*;
use crate::node::content_graph::ManifestState;
use crate::sync::{Host, Staged};
use locust_proto::api::{ApiError, ErrorCode};
use locust_proto::event::Body;
use locust_proto::id::{BlobHash, EventId};
use locust_proto::manifest::{Entry, Manifest};
use locust_proto::store::{Blob, LocalWrite};
use locust_proto::{seal, sync::SyncMessage};

fn put(peer: &mut Peer, goal: GoalId, bytes: &[u8]) -> BlobHash {
    let Response::BlobStored { hash } = peer.call(Request::BlobPut {
        goal,
        bytes: bytes.to_vec(),
    }) else {
        panic!("put")
    };
    hash
}
fn file(path: &str, hash: BlobHash, size: usize) -> Entry {
    Entry {
        path: path.into(),
        executable: false,
        size: size as u64,
        content: hash,
    }
}
fn manifest(peer: &mut Peer, goal: GoalId, entries: Vec<Entry>) -> BlobHash {
    put(peer, goal, &Manifest { entries }.encode().unwrap())
}
fn propose(peer: &mut Peer, goal: GoalId, input: BlobHash) {
    peer.call(Request::TaskOpen {
        goal,
        text: "snapshot".into(),
        task_type: None,
        inputs: BTreeMap::from([("snapshot".into(), input)]),
        parent: None,
    });
}
fn rounds(peers: &mut [Peer]) -> Vec<SyncMessage> {
    (1..=6)
        .flat_map(|round| reconcile(peers, 40_000 * round))
        .collect()
}
fn get(peer: &mut Peer, goal: GoalId, hash: BlobHash) -> Result<Vec<u8>, ApiError> {
    let Step::Reply(reply) = peer.node.request(
        ConnId(1),
        RequestFrame {
            id: 999,
            idempotency: None,
            on_behalf: Some(peer.principal),
            request: Request::BlobGet { goal, hash },
        },
        300_000,
    ) else {
        panic!("reply")
    };
    match reply.result? {
        Response::Blob { bytes } => Ok(bytes),
        _ => panic!("blob"),
    }
}
fn publish(peers: &mut [Peer], goal: GoalId, body: Body) {
    let event = reference_event(&peers[0], goal, None, body);
    for peer in peers {
        assert_eq!(
            Host::replica(&mut peer.node, &goal)
                .unwrap()
                .receive(vec![event.to_wire()]),
            Ok(1)
        );
    }
}
fn root(input: BlobHash) -> Body {
    Body::TaskOpened {
        binding: locust_proto::event::TaskBinding {
            rules: EventId([0; 32]),
            task_type: None,
            inputs: BTreeMap::from([("snapshot".into(), input)]),
            parent: None,
            stage: None,
        },
    }
}
fn workspace_root(input: BlobHash) -> Body {
    Body::WorkspaceProposed {
        context: Context {
            scope: Scope::Workspace,
            round: EventId([0; 32]),
        },
        parent: None,
        result_manifest: input,
        sources: Vec::new(),
    }
}
fn manifest_state(peer: &Peer, goal: GoalId, hash: BlobHash) -> Result<ManifestState, ApiError> {
    peer.node
        .workspace_manifest(&peer.node.goals[&goal], hash, Some(&peer.principal))
}
fn stage(peer: &mut Peer, goal: GoalId, blob: &Blob) -> Staged {
    Host::replica(&mut peer.node, &goal).unwrap().stage(
        &blob.hash(),
        0,
        blob.bytes().len() as u64,
        blob.bytes(),
    )
}

#[test]
fn task_manifest_files_replicate_reopen_and_withdraw_without_sniffing_leaves() {
    let mut peers = [Peer::new(1), Peer::new(2)];
    let goal = found(&mut peers);
    let hidden = put(&mut peers[0], goal, b"must not follow file bytes");
    let nested_plain = Manifest {
        entries: vec![file("hidden", hidden, 26)],
    }
    .encode()
    .unwrap();
    let nested = put(&mut peers[0], goal, &nested_plain);
    // Select a manifest above its child in hash order. The driver's cursor has
    // already passed this child when the container arrives.
    let (snapshot, data, plain) = (0..1000)
        .find_map(|n| {
            let plain = format!("file {n}");
            let data = put(&mut peers[0], goal, plain.as_bytes());
            let snapshot = manifest(
                &mut peers[0],
                goal,
                vec![
                    file("a", data, plain.len()),
                    file("nested", nested, nested_plain.len()),
                ],
            );
            (data < snapshot).then_some((snapshot, data, plain))
        })
        .expect("deterministic lower child hash");
    propose(&mut peers[0], goal, snapshot);
    let sent = rounds(&mut peers);
    assert!(
        sent.iter()
            .any(|frame| matches!(frame, SyncMessage::BlobRequest { hash, .. } if *hash == data))
    );
    assert_eq!(get(&mut peers[1], goal, data).unwrap(), plain.as_bytes());
    assert_eq!(get(&mut peers[1], goal, nested).unwrap(), nested_plain);
    assert_eq!(
        get(&mut peers[1], goal, hidden).unwrap_err().code,
        ErrorCode::NotFound
    );
    assert!(peers[1].store.blob_len(&hidden).unwrap().is_none());
    assert!(
        crate::node::requests::content::blob_record(&peers[1].store, &goal, &data)
            .unwrap()
            .is_none()
    );
    peers[1].restart();
    assert_eq!(get(&mut peers[1], goal, data).unwrap(), plain.as_bytes());
    assert!(
        Host::replica(&mut peers[1].node, &goal)
            .unwrap()
            .blob_len(&data)
            .is_some()
    );
    peers[1].call(Request::BlobWithdraw {
        goal,
        hash: snapshot,
    });
    assert_eq!(
        get(&mut peers[1], goal, data).unwrap_err().code,
        ErrorCode::NotFound
    );
    assert!(
        Host::replica(&mut peers[1].node, &goal)
            .unwrap()
            .blob_len(&data)
            .is_none()
    );
    peers[1].restart();
    assert_eq!(
        get(&mut peers[1], goal, data).unwrap_err().code,
        ErrorCode::NotFound
    );
    let alternative = manifest(
        &mut peers[0],
        goal,
        vec![file("renamed", data, plain.len())],
    );
    propose(&mut peers[0], goal, alternative);
    rounds(&mut peers);
    assert_eq!(get(&mut peers[1], goal, data).unwrap(), plain.as_bytes());
}

#[test]
fn workspace_result_manifest_is_typed_and_generic_artifacts_stay_opaque() {
    let mut peers = [Peer::new(1), Peer::new(2)];
    let goal = found(&mut peers);
    let shared = put(&mut peers[0], goal, b"shared");
    let unrelated = put(&mut peers[0], goal, b"unrelated");
    let artifact = manifest(&mut peers[0], goal, vec![file("secret", unrelated, 9)]);
    let snapshot = manifest(&mut peers[0], goal, vec![file("a", shared, 6)]);
    publish(&mut peers, goal, workspace_root(snapshot));
    publish(
        &mut peers,
        goal,
        Body::ContributionPublished {
            context: Context {
                scope: Scope::Goal,
                round: EventId([0; 32]),
            },
            attempt: None,
            sources: Vec::new(),
            artifacts: vec![artifact],
        },
    );
    rounds(&mut peers);
    assert_eq!(get(&mut peers[1], goal, shared).unwrap(), b"shared");
    assert!(peers[1].store.blob_len(&artifact).unwrap().is_some());
    assert_eq!(
        get(&mut peers[1], goal, unrelated).unwrap_err().code,
        ErrorCode::NotFound
    );
    assert!(matches!(
        manifest_state(&peers[1], goal, snapshot).unwrap(),
        ManifestState::Ready { .. }
    ));
    peers[1].restart();
    assert_eq!(get(&mut peers[1], goal, shared).unwrap(), b"shared");
}

#[test]
fn late_key_expands_held_manifest_after_restart() {
    let mut peers = [Peer::new(1), Peer::new(2)];
    let goal = found(&mut peers);
    let data = put(&mut peers[0], goal, b"late key");
    let input = manifest(&mut peers[0], goal, vec![file("a", data, 8)]);
    publish(&mut peers, goal, root(input));
    let key = peers[0].node.goals[&goal].keys[&0];
    let LocalWrite::Put {
        space,
        key: record_key,
        ..
    } = crate::node::entry::key_write(&goal, 0, &key)
    else {
        unreachable!()
    };
    let mut tx = crate::node::commit::Tx::none();
    tx.local(LocalWrite::Delete {
        space,
        key: record_key,
    })
    .touch(goal);
    peers[1].node.land(tx).unwrap();
    let sealed = Blob::new(peers[0].store.blob(&input).unwrap().unwrap());
    assert_eq!(stage(&mut peers[1], goal, &sealed), Staged::Complete);
    assert_eq!(
        get(&mut peers[1], goal, data).unwrap_err().code,
        ErrorCode::NotFound
    );
    peers[1].restart();
    assert_eq!(
        get(&mut peers[1], goal, data).unwrap_err().code,
        ErrorCode::NotFound
    );
    assert!(
        Host::replica(&mut peers[1].node, &goal)
            .unwrap()
            .offer_key(0, key)
    );
    assert!(peers[1].node.outbound.contains(&goal));
    rounds(&mut peers);
    assert_eq!(get(&mut peers[1], goal, data).unwrap(), b"late key");
}

#[test]
fn conflicting_manifest_lengths_keep_valid_reference_and_reject_inflation() {
    let mut peers = [Peer::new(1), Peer::new(2)];
    let goal = found(&mut peers);
    let data = put(&mut peers[0], goal, b"valid");
    let impossible = BlobHash([77; 32]);
    let invalid = manifest(
        &mut peers[0],
        goal,
        vec![
            file("a", data, 99),
            Entry {
                size: u64::MAX,
                ..file("overflow", impossible, 0)
            },
        ],
    );
    publish(&mut peers, goal, root(invalid));
    let sealed = Blob::new(peers[0].store.blob(&invalid).unwrap().unwrap());
    assert_eq!(stage(&mut peers[1], goal, &sealed), Staged::Complete);
    assert_eq!(
        get(&mut peers[1], goal, impossible).unwrap_err().code,
        ErrorCode::NotFound
    );
    let data_blob = Blob::new(peers[0].store.blob(&data).unwrap().unwrap());
    assert_eq!(stage(&mut peers[1], goal, &data_blob), Staged::Rejected);
    let valid = manifest(&mut peers[0], goal, vec![file("a", data, 5)]);
    publish(&mut peers, goal, root(valid));
    let sealed = Blob::new(peers[0].store.blob(&valid).unwrap().unwrap());
    assert_eq!(stage(&mut peers[1], goal, &sealed), Staged::Complete);
    assert_eq!(
        Host::replica(&mut peers[1].node, &goal).unwrap().stage(
            &data,
            0,
            data_blob.bytes().len() as u64 + 1000,
            &data_blob.bytes()[..10]
        ),
        Staged::Rejected
    );
    assert_eq!(peers[1].store.staged_len(&data).unwrap(), 0);
    assert_eq!(stage(&mut peers[1], goal, &data_blob), Staged::Complete);
    assert_eq!(get(&mut peers[1], goal, data).unwrap(), b"valid");
}

#[test]
fn removed_principal_and_viewer_need_an_older_path_even_for_an_old_file() {
    let mut peers = [Peer::new(1), Peer::new(2)];
    let goal = found(&mut peers);
    let removed = peers[1].principal;
    let Response::AgentEnrolled { agent: current } = request(
        &mut peers[1].node,
        None,
        Request::AgentEnroll {
            name: "cohost".into(),
            grants: Grants { manage_goals: true },
            credential: Credential([31; 32]).digest(),
        },
    ) else {
        panic!("enroll")
    };
    let Response::Invited { ticket } = peers[0].call(Request::GoalInvite {
        goal,
        expires_ms: None,
    }) else {
        panic!("invite")
    };
    request(
        &mut peers[1].node,
        Some(current),
        Request::GoalJoin { ticket },
    );
    rounds(&mut peers);
    let viewer = Credential([32; 32]);
    request(
        &mut peers[1].node,
        None,
        Request::ViewerEnroll {
            agent: removed,
            credential: viewer.digest(),
        },
    );
    let old_file = put(&mut peers[0], goal, b"old bytes");
    let old_manifest = manifest(&mut peers[0], goal, vec![file("old", old_file, 9)]);
    let wrong_old_manifest = manifest(&mut peers[0], goal, vec![file("wrong", old_file, 99)]);
    peers[0].call(Request::MemberRemove {
        goal,
        member: removed,
    });
    let new_manifest = manifest(&mut peers[0], goal, vec![file("new", old_file, 9)]);
    propose(&mut peers[0], goal, new_manifest);
    propose(&mut peers[0], goal, wrong_old_manifest);
    rounds(&mut peers);
    assert!(peers[1].node.goals[&goal].keys.contains_key(&1));
    // The host has already decoded this manifest using its remaining member's
    // key. A removed reader still cannot consume that cached new-epoch tree.
    assert_eq!(
        manifest_state(&peers[1], goal, new_manifest)
            .unwrap_err()
            .code,
        ErrorCode::Denied
    );
    assert_eq!(
        get(&mut peers[1], goal, old_file).unwrap_err().code,
        ErrorCode::Denied
    );
    peers[1].principal = current;
    assert!(matches!(
        manifest_state(&peers[1], goal, new_manifest).unwrap(),
        ManifestState::Ready { .. }
    ));
    assert_eq!(get(&mut peers[1], goal, old_file).unwrap(), b"old bytes");
    peers[1].restart();
    assert!(matches!(
        peers[1].node.connect(
            ConnId(9),
            &ClientHello {
                api_version: locust_proto::API_VERSION,
                credential: viewer,
                session: None
            },
            0
        ),
        ServerHello::Welcome { .. }
    ));
    let viewer_get = |peer: &mut Peer| {
        let Step::Reply(reply) = peer.node.request(
            ConnId(9),
            RequestFrame {
                id: 100,
                idempotency: None,
                on_behalf: None,
                request: Request::BlobGet {
                    goal,
                    hash: old_file,
                },
            },
            0,
        ) else {
            panic!("reply")
        };
        reply.result
    };
    assert_eq!(
        viewer_get(&mut peers[1]).unwrap_err().code,
        ErrorCode::Denied
    );
    propose(&mut peers[0], goal, old_manifest);
    rounds(&mut peers);
    assert!(
        matches!(viewer_get(&mut peers[1]), Ok(Response::Blob { bytes }) if bytes == b"old bytes")
    );
    peers[1].principal = removed;
    assert_eq!(get(&mut peers[1], goal, old_file).unwrap(), b"old bytes");
    peers[1].principal = current;
    peers[1].call(Request::BlobWithdraw {
        goal,
        hash: old_manifest,
    });
    peers[1].principal = removed;
    assert_eq!(
        get(&mut peers[1], goal, old_file).unwrap_err().code,
        ErrorCode::Denied
    );
    assert_eq!(
        viewer_get(&mut peers[1]).unwrap_err().code,
        ErrorCode::Denied
    );
    peers[1].restart();
    assert_eq!(
        get(&mut peers[1], goal, old_file).unwrap_err().code,
        ErrorCode::Denied
    );
}

#[test]
fn unauthenticated_or_noncanonical_containers_cannot_authorize_children() {
    let mut peers = [Peer::new(1), Peer::new(2)];
    let goal = found(&mut peers);
    let child = put(&mut peers[0], goal, b"hidden");
    let plain = Manifest {
        entries: vec![file("a", child, 6)],
    }
    .encode()
    .unwrap();
    let mut noncanonical = plain.clone();
    noncanonical.push(0);
    let invalid = put(&mut peers[0], goal, &noncanonical);
    publish(&mut peers, goal, root(invalid));
    let invalid_blob = Blob::new(peers[0].store.blob(&invalid).unwrap().unwrap());
    assert_eq!(stage(&mut peers[1], goal, &invalid_blob), Staged::Complete);
    let foreign = Blob::new(
        seal::seal(
            &GoalId([90; 32]),
            0,
            &peers[0].node.goals[&goal].keys[&0],
            &plain,
        )
        .unwrap(),
    );
    publish(&mut peers, goal, root(foreign.hash()));
    assert_eq!(stage(&mut peers[1], goal, &foreign), Staged::Complete);
    assert_eq!(
        get(&mut peers[1], goal, child).unwrap_err().code,
        ErrorCode::NotFound
    );
    peers[1].restart();
    assert_eq!(
        get(&mut peers[1], goal, child).unwrap_err().code,
        ErrorCode::NotFound
    );
    assert!(
        Host::replica(&mut peers[1].node, &goal)
            .unwrap()
            .blob_len(&child)
            .is_none()
    );
}

#[test]
fn descendant_epoch_is_bounded_by_container_not_only_event_epoch() {
    let mut peers = [Peer::new(1), Peer::new(2), Peer::new(3)];
    let goal = found(&mut peers);
    let epoch_zero_key = peers[0].node.goals[&goal].keys[&0];
    let removed = peers[2].principal;
    peers[0].call(Request::MemberRemove {
        goal,
        member: removed,
    });
    let child = put(&mut peers[0], goal, b"new epoch");
    let plain = Manifest {
        entries: vec![file("a", child, 9)],
    }
    .encode()
    .unwrap();
    let old_container = Blob::new(seal::seal(&goal, 0, &epoch_zero_key, &plain).unwrap());
    rounds(&mut peers);
    // The signed root permits epoch one; its containing manifest permits zero.
    publish(&mut peers[..2], goal, root(old_container.hash()));
    assert_eq!(stage(&mut peers[1], goal, &old_container), Staged::Complete);
    let child_blob = Blob::new(peers[0].store.blob(&child).unwrap().unwrap());
    assert_eq!(stage(&mut peers[1], goal, &child_blob), Staged::Rejected);
    assert!(peers[1].store.blob_len(&child).unwrap().is_none());
}

#[test]
fn complete_workspace_snapshots_reuse_unchanged_opaque_files() {
    let mut peers = [Peer::new(1), Peer::new(2)];
    let goal = found(&mut peers);
    let unchanged = put(&mut peers[0], goal, b"base");
    let second = put(&mut peers[0], goal, b"head");
    let first = manifest(&mut peers[0], goal, vec![file("a", unchanged, 4)]);
    let complete = manifest(
        &mut peers[0],
        goal,
        vec![file("a", unchanged, 4), file("b", second, 4)],
    );
    publish(&mut peers, goal, workspace_root(first));
    publish(&mut peers, goal, workspace_root(complete));
    rounds(&mut peers);
    assert_eq!(get(&mut peers[1], goal, unchanged).unwrap(), b"base");
    assert_eq!(get(&mut peers[1], goal, second).unwrap(), b"head");
    peers[1].call(Request::BlobWithdraw { goal, hash: first });
    assert_eq!(get(&mut peers[1], goal, unchanged).unwrap(), b"base");
}

#[test]
fn manifest_readiness_distinguishes_missing_key_invalid_and_withdrawn() {
    let mut peers = [Peer::new(1), Peer::new(2)];
    let goal = found(&mut peers);
    let absent = BlobHash([91; 32]);
    publish(&mut peers, goal, workspace_root(absent));
    assert!(matches!(
        manifest_state(&peers[1], goal, absent).unwrap(),
        ManifestState::Missing
    ));
    let child = put(&mut peers[0], goal, b"file");
    let snapshot = manifest(&mut peers[0], goal, vec![file("a", child, 4)]);
    publish(&mut peers, goal, workspace_root(snapshot));
    let key = peers[0].node.goals[&goal].keys[&0];
    let LocalWrite::Put {
        space,
        key: record_key,
        ..
    } = crate::node::entry::key_write(&goal, 0, &key)
    else {
        unreachable!()
    };
    let mut tx = crate::node::commit::Tx::none();
    tx.local(LocalWrite::Delete {
        space,
        key: record_key,
    })
    .touch(goal);
    peers[1].node.land(tx).unwrap();
    let sealed = Blob::new(peers[0].store.blob(&snapshot).unwrap().unwrap());
    assert_eq!(stage(&mut peers[1], goal, &sealed), Staged::Complete);
    assert!(matches!(
        manifest_state(&peers[1], goal, snapshot).unwrap(),
        ManifestState::KeyMissing { epoch: 0 }
    ));
    assert!(
        Host::replica(&mut peers[1].node, &goal)
            .unwrap()
            .offer_key(0, key)
    );
    assert!(matches!(
        manifest_state(&peers[1], goal, snapshot).unwrap(),
        ManifestState::Ready { .. }
    ));
    let mut invalid = Manifest {
        entries: vec![file("a", child, 4)],
    }
    .encode()
    .unwrap();
    invalid.push(0);
    let invalid = put(&mut peers[0], goal, &invalid);
    publish(&mut peers, goal, workspace_root(invalid));
    let sealed = Blob::new(peers[0].store.blob(&invalid).unwrap().unwrap());
    assert_eq!(stage(&mut peers[1], goal, &sealed), Staged::Complete);
    assert!(matches!(
        manifest_state(&peers[1], goal, invalid).unwrap(),
        ManifestState::Invalid { .. }
    ));
    let before = peers[1].node.blob_index.stats(&goal);
    manifest_state(&peers[1], goal, invalid).unwrap();
    manifest_state(&peers[1], goal, invalid).unwrap();
    let after = peers[1].node.blob_index.stats(&goal);
    assert_eq!(before.manifest_decodes, after.manifest_decodes);
    assert_eq!(after.cache_hits - before.cache_hits, 2);
    peers[1].call(Request::BlobWithdraw {
        goal,
        hash: snapshot,
    });
    assert!(matches!(
        manifest_state(&peers[1], goal, snapshot).unwrap(),
        ManifestState::Withdrawn
    ));
    peers[1].restart();
    assert!(matches!(
        manifest_state(&peers[1], goal, snapshot).unwrap(),
        ManifestState::Withdrawn
    ));
}

#[test]
fn growing_workspace_history_does_not_redecode_or_rescan_on_unrelated_work() {
    let mut peers = [Peer::new(1), Peer::new(2)];
    let goal = found(&mut peers);
    let data = put(&mut peers[0], goal, b"same");
    let started = std::time::Instant::now();
    for size in 1..=128 {
        let entries = (0..size)
            .map(|index| file(&format!("file-{index:04}"), data, 4))
            .collect();
        let snapshot = manifest(&mut peers[0], goal, entries);
        publish(&mut peers, goal, workspace_root(snapshot));
        let sealed = Blob::new(peers[0].store.blob(&snapshot).unwrap().unwrap());
        assert_eq!(stage(&mut peers[1], goal, &sealed), Staged::Complete);
        if [8, 32, 128].contains(&size) {
            let before = peers[1].node.blob_index.stats(&goal);
            for _ in 0..3 {
                publish(
                    &mut peers,
                    goal,
                    Body::ContributionPublished {
                        context: Context {
                            scope: Scope::Goal,
                            round: EventId([0; 32]),
                        },
                        attempt: None,
                        sources: Vec::new(),
                        artifacts: vec![],
                    },
                );
            }
            let after = peers[1].node.blob_index.stats(&goal);
            assert_eq!(before.history_rebuilds, after.history_rebuilds);
            assert_eq!(before.manifest_decodes, after.manifest_decodes);
            assert_eq!(after.event_roots - before.event_roots, 3);
            eprintln!(
                "workspace-index trees={size} entries={} decodes={} unrelated_root_visits={} rebuilds={} elapsed_ms={}",
                size * (size + 1) / 2,
                after.manifest_decodes,
                after.event_roots - before.event_roots,
                after.history_rebuilds,
                started.elapsed().as_millis()
            );
        }
    }
    assert_eq!(peers[1].node.blob_index.stats(&goal).manifest_decodes, 128);
    let before = peers[0].node.blob_index.stats(&goal);
    let removed = peers[1].principal;
    peers[0].call(Request::MemberRemove {
        goal,
        member: removed,
    });
    rounds(&mut peers);
    let after = peers[0].node.blob_index.stats(&goal);
    assert!(after.history_rebuilds > before.history_rebuilds);
    assert_eq!(after.manifest_decodes, before.manifest_decodes);
    assert!(after.cache_hits > before.cache_hits);
}

#[test]
fn cache_does_not_authorize_dropped_manifest_or_replaced_key() {
    let mut peers = [Peer::new(1), Peer::new(2)];
    let goal = found(&mut peers);
    let data = put(&mut peers[0], goal, b"data");
    let snapshot = manifest(&mut peers[0], goal, vec![file("a", data, 4)]);
    publish(&mut peers, goal, workspace_root(snapshot));
    rounds(&mut peers);
    assert!(matches!(
        manifest_state(&peers[1], goal, snapshot).unwrap(),
        ManifestState::Ready { .. }
    ));
    let before = peers[1].node.blob_index.stats(&goal);
    let key = peers[1].node.goals[&goal].keys[&0];
    let mut tx = crate::node::commit::Tx::none();
    tx.local(crate::node::entry::key_write(
        &goal,
        0,
        &locust_proto::crypto::ContentKey([99; 32]),
    ))
    .touch(goal);
    peers[1].node.land(tx).unwrap();
    assert!(!matches!(
        manifest_state(&peers[1], goal, snapshot),
        Ok(ManifestState::Ready { .. })
    ));
    assert!(get(&mut peers[1], goal, data).is_err());
    let mut tx = crate::node::commit::Tx::none();
    tx.local(crate::node::entry::key_write(&goal, 0, &key))
        .touch(goal);
    peers[1].node.land(tx).unwrap();
    assert!(matches!(
        manifest_state(&peers[1], goal, snapshot).unwrap(),
        ManifestState::Ready { .. }
    ));
    assert_eq!(
        peers[1].node.blob_index.stats(&goal).manifest_decodes,
        before.manifest_decodes
    );
    let mut tx = crate::node::commit::Tx::none();
    tx.commit.drop_blobs.push(snapshot);
    tx.touch(goal);
    peers[1].node.land(tx).unwrap();
    assert!(matches!(
        manifest_state(&peers[1], goal, snapshot).unwrap(),
        ManifestState::Missing
    ));
    assert_eq!(
        get(&mut peers[1], goal, data).unwrap_err().code,
        ErrorCode::NotFound
    );
    let sealed = Blob::new(peers[0].store.blob(&snapshot).unwrap().unwrap());
    assert_eq!(stage(&mut peers[1], goal, &sealed), Staged::Complete);
    assert_eq!(get(&mut peers[1], goal, data).unwrap(), b"data");
    assert_eq!(
        peers[1].node.blob_index.stats(&goal).manifest_decodes,
        before.manifest_decodes
    );
}
