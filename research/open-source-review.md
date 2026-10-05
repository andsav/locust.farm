# Public source review

Reviewed 2026-10-04, starting from `d426989`. Scope: the local tracked tree and
all locally reachable Git history. This is a source and publication review,
not a runtime security assessment or a full dependency-license audit.

## Privacy findings

The initial tree contained 786 tracked files. The history scan covered 261
reachable commits across 27 local refs, 2,795 unique blobs and approximately
47 MB of text. Pattern searches and manual inspection found no recognized live
API credentials, private-key blocks, provider/AWS/GitHub/Slack tokens, JWTs,
Basic Auth passwords or hashes, or real invitation tickets. No environment files,
private keys, credential databases or executable build artifacts were tracked.
Dedicated secret scanners were unavailable; these observations do not guarantee
that every possible secret format was detected.

The actionable disclosures were an operator's production SSH destination and
personal absolute paths in captured commands and logs. The deployment script now
requires an explicit `LOCUST_DEPLOY_SERVER`. Retained reports redact identifying
paths and the deployment destination. Public signing identities, public keys,
artifact hashes and deterministic test vectors remain intact.

The binary artifacts were 23 PNG files. Representative images showed cropped
site/mockup content and synthetic participants, without personal browser chrome,
accounts or private chats; every pixel of every image was not separately reviewed.

## Public relevance

The retained [research index](README.md) selects current design questions,
reproducible experiments and supporting evidence. Superseded protocol campaigns,
internal takeover/review records, obsolete source snapshots and private-project
implementation studies were removed. A local archive outside the repository
preserves the original documents and evidence.

Current formal models/configurations, experiment runners, public guides and
required fault-test helpers remain. Earlier artifacts retained for a relevant
experiment identify their source and limits; they do not qualify the current
API 5 runtime. The published terminal preview is separately identified in the
[release record](../docs/public-preview-release.md).

The README now leads with user capabilities, explains formations, and gives
source build/start instructions. Current-source and published-preview state are
kept distinct. The copied Phosphor icon paths now have a full permission notice
in [third-party notices](../THIRD_PARTY_NOTICES.md).

## Verification of the cleanup

Documentation indexes/local links and staged whitespace checks passed. The
Python 3.12 helper suite completed 253 tests with three explicit skips and no failures. With Node
24.14.1, site lint, type checks, all 187 unit tests and the static production
build passed. Postbuild checked 91 routes, 19 raw articles and 10 exact assets,
including links and anchors.

All four executable manual recipes passed against the selected existing API-5
development binary. This was not a new Rust build or release qualification.
The deployment script passed Bash syntax checking and refused an omitted SSH
destination before any build or network operation. All 17 retained evidence JSON
files parsed, and all 146 removed files were verified present in the local archive.

## History still requires a publication choice

Deleting files and redacting the current tree does not remove earlier commits.
The scan found the deployment address in 13 historical blobs and identifying
personal paths in 35. Earlier commits also retain the removed private-project
research and internal records.

Publish a clean export of the reviewed tree if that history should remain
private, or separately review and authorize a history-cleanup procedure before
changing repository visibility. No history rewrite, push or visibility change
was performed by this review. The scan does not cover remote refs absent from
the local repository, GitHub attachments, release uploads, issues or CI logs.
