use super::*;
use locust_proto::organization::{presets, semantic_hash};
use locust_proto::store::MemStore;

const OWNER: PublicKey = PublicKey([1; 32]);
const OTHER: PublicKey = PublicKey([2; 32]);
const SOURCE: &str = "{\"schema_version\":1}";
fn apply<T>(store: &mut MemStore, prepared: Prepared<T>) -> T {
    store.commit(&prepared.commit).unwrap();
    prepared.value
}
fn create(store: &mut MemStore, id: &str, source: &str) -> Draft {
    let prepared = prepare_create(store, OWNER, id, 0, source.into()).unwrap();
    apply(store, prepared)
}
#[test]
fn invalid_drafts_survive_reopen_and_owner_scope_isolated() {
    let mut store = MemStore::new();
    let created = create(&mut store, "draft", "{");
    assert_eq!(created.revision, 1);
    let reopened = store.reopen();
    assert_eq!(draft(&reopened, OWNER, "draft").unwrap(), created);
    assert!(matches!(
        draft(&reopened, OTHER, "draft"),
        Err(Error::NotFound)
    ));
    assert!(drafts(&reopened, OTHER).unwrap().is_empty());
    assert!(matches!(
        prepare_publish(&store, OWNER, "draft", "published", 1, &created.source_hash),
        Err(Error::InvalidDocument { .. })
    ));
    assert!(publications(&store, OWNER).unwrap().is_empty());
    let independent = prepare_create(&store, OTHER, "draft", 0, SOURCE.into()).unwrap();
    store.commit(&independent.commit).unwrap();
    assert_eq!(drafts(&store, OWNER).unwrap().len(), 1);
    assert_eq!(drafts(&store, OTHER).unwrap().len(), 1);
}
#[test]
fn stale_source_revision_and_hash_return_current_edit() {
    let mut store = MemStore::new();
    let original = create(&mut store, "draft", SOURCE);
    let prepared = prepare_update(
        &store,
        OWNER,
        "draft",
        1,
        "{ \"schema_version\": 1 }".into(),
    )
    .unwrap();
    let updated = apply(&mut store, prepared);
    assert_ne!(original.source_hash, updated.source_hash);
    for (revision, hash) in [
        (1, original.source_hash.as_str()),
        (2, original.source_hash.as_str()),
    ] {
        let result = prepare_publish(&store, OWNER, "draft", "published", revision, hash);
        assert!(matches!(result, Err(Error::SourceConflict { current }) if *current == updated));
    }
    assert!(
        matches!(prepare_update(&store, OWNER, "draft", 1, "local edit".into()), Err(Error::SourceConflict { current }) if *current == updated)
    );
}
#[test]
fn publication_is_immutable_and_retry_uses_pinned_identity() {
    let mut store = MemStore::new();
    let original = create(&mut store, "draft", SOURCE);
    let prepared = prepare_publish(
        &store,
        OWNER,
        "draft",
        "published",
        1,
        &original.source_hash,
    )
    .unwrap();
    let published = apply(&mut store, prepared);
    let prepared = prepare_update(
        &store,
        OWNER,
        "draft",
        1,
        "{ \"schema_version\": 1 }".into(),
    )
    .unwrap();
    let updated = apply(&mut store, prepared);
    let retry = prepare_publish(
        &store,
        OWNER,
        "draft",
        "published",
        1,
        &original.source_hash,
    )
    .unwrap();
    assert_eq!(retry.value, published);
    assert_eq!(retry.commit, Commit::default());
    assert!(
        matches!(prepare_publish(&store, OWNER, "draft", "published", 2, &updated.source_hash), Err(Error::PublicationConflict { current }) if *current == published)
    );
    let prepared =
        prepare_publish(&store, OWNER, "draft", "second", 2, &updated.source_hash).unwrap();
    let second = apply(&mut store, prepared);
    assert_ne!(published.source_hash, second.source_hash);
    assert_eq!(published.semantic_hash, second.semantic_hash);
    assert_eq!(
        publication(&store.reopen(), OWNER, "published").unwrap(),
        published
    );
    assert!(matches!(
        publication(&store, OTHER, "published"),
        Err(Error::NotFound)
    ));
}
#[test]
fn presentation_cas_never_changes_source_or_semantic_identity() {
    let mut store = MemStore::new();
    let created = create(&mut store, "draft", SOURCE);
    let prepared =
        prepare_publish(&store, OWNER, "draft", "published", 1, &created.source_hash).unwrap();
    let published = apply(&mut store, prepared);
    let prepared =
        prepare_presentation(&store, OWNER, "draft", 0, "{\"x\":42,\"nodes\":[]}".into()).unwrap();
    let layout = apply(&mut store, prepared);
    assert_eq!(layout.revision, 1);
    assert_eq!(draft(&store, OWNER, "draft").unwrap(), created);
    assert_eq!(publication(&store, OWNER, "published").unwrap(), published);
    assert_eq!(
        semantic_hash(&published.normalized().unwrap()),
        published.semantic_hash
    );
    assert!(
        matches!(prepare_presentation(&store, OWNER, "draft", 0, "{}".into()), Err(Error::PresentationConflict { current }) if *current == layout)
    );
    assert!(matches!(
        prepare_presentation(&store, OWNER, "draft", 1, "{".into()),
        Err(Error::Invalid(_))
    ));
    assert!(matches!(
        prepare_presentation(&store, OTHER, "draft", 0, "{}".into()),
        Err(Error::NotFound)
    ));
}
#[test]
fn all_reusable_presets_publish_without_bound_members() {
    let mut store = MemStore::new();
    for preset in presets() {
        let source = serde_json::to_string(&preset.formation).unwrap();
        let created = create(&mut store, &preset.name, &source);
        let prepared = prepare_publish(
            &store,
            OWNER,
            &preset.name,
            &preset.name,
            1,
            &created.source_hash,
        )
        .unwrap();
        apply(&mut store, prepared);
    }
    assert_eq!(publications(&store, OWNER).unwrap().len(), 6);
}
