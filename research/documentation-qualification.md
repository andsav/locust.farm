# Current manual recipe qualification

Date: 2026-10-04. **Status: local development CLI/daemon evidence.** These checks
exercise the exact executable Bash fences in the public manual against one
identified binary on macOS arm64. They are not native-model, multi-machine,
public-release qualification. The recipe checks are distinct from the separately
observed disposable test-signed installation described below.

## Exact installed protocol 2 candidate

All four current recipes passed against test-signed installed source commit
`0a295cdabe6a878cc733c791ca73863933cfa45a`, binary SHA256
`b577709eb981544b4bcf5bdcc5efd677a6133d24e474ab274a1fe500215450b3`.
The fourth recipe, separate-goal export, proves explicit selected-context copying
and isolated goal membership. The installed matching 40-source manual was read
without checkout access. Exact recipe hashes and verdicts are retained in
[the current native qualification](organization-native-qualification.md) and its
[structured evidence](evidence/organization-native-qualification/results.json).
This campaign used an explicit 300-second per-recipe watchdog; all four exited
successfully. It remains a local scripted campaign, with no real model calls or
physical multi-machine qualification.

## Historical three-recipe checks and identity

The [sanitized record](evidence/documentation-recipes-2026-10-04.json) identifies
binary SHA-256, its reported build label, each exact recipe hash, exit status and
assertion output. The binary reports an `e95e27729f55-dirty` build label because
it was built while the later runtime changes were uncommitted; the exact digest,
not that older HEAD alone, identifies the tested bytes. Those changes were then
committed as `f985cf9` and `b08d797`. The checker also confirms the binary did not
change during the campaign.

The [checker](../scripts/check_documentation.py) runs only marked, closed Bash
fences from canonical guide files, each with a fresh private temporary directory.
The explicit `--timeout 60` watchdog bounds this test invocation; it is not a
runtime work budget. Discovery and relays are disabled and the daemon binds to
loopback. No native model or external provider is invoked. Credentials and
invitation values are private local inputs; retained output contains no secrets.

- [Collaboration](../docs/guide/collaboration.md) proves same-daemon invitation and
  admission, taskless declaration, two distinct task attempts, exact peer review,
  pinned task rules after changing future defaults, and state reopening.
- [Private authoring](../docs/guide/formation-authoring.md) proves exact revision/hash
  publication, recovery information for a stale edit, and refusal of goal creation
  through an author-only credential.
- [Open patch application](../docs/guide/apply.md) proves normal application refuses
  an unselected taskless contribution; an explicit authenticated owner choice
  applies the exact file while the contribution remains unapproved/unselected.

All three passed. Parser unit checks reject malformed, unclosed, empty and
ambiguous marked recipes. CI repeats the exact recipes against its built binary.
The site gates pass lint, Svelte checks with zero errors/warnings, 37 tests and
production prerender validation of 88 routes, 18 raw articles and ten assets.
A site build is not a deployed website. The active implementation audit and
[status ledger](https://github.com/andsav/locust.farm/blob/ddb2db1e609652a1de766b453d8e86b45b25a1f3/docs/formations-status.md) retain remaining
client/provider, transport and release boundaries separately.
