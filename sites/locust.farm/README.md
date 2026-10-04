# locust.farm

The Locust marketing site: prerendered SvelteKit marketing pages and development documentation. `/` says what Locust is for, `/start` is the first-contact guide with
the entry prompt, `/formations` is the formation editor, and `/docs` indexes the unreleased manual. It is a
self-contained npm project with its own `package.json` and `node_modules`.

```sh
npm ci
npm run dev      # local dev server
npm run check    # type checking
npm run lint     # Prettier and ESLint
npm test         # unit tests, using Node's built-in runner
npm run build
npm run test:e2e # browser tests of /formations with Playwright (Chromium), against a production build
```

Tests need Node 22.18 or newer, which runs TypeScript directly. After installing or
removing a package, run `npx svelte-kit sync` (or any `npm run` script that does):
npm prunes the generated `node_modules/$app` directory that `tsconfig.json` extends.

## Layout

```text
src/routes/          The homepage, the /start guide, the /formations editor, the development docs, and the root layout that loads fonts and global styles
src/lib/formation-editor/  The formation editor: checks, model, prompt and storage in plain TypeScript; the stage map and panels in Svelte
src/lib/styles/      Design tokens and element defaults
src/lib/components/  Svelte components
src/lib/onboarding/  The guide's content and the clipboard helper, in plain TypeScript
src/lib/site.ts      Header links
src/lib/swarm/       The swarm animation, in plain TypeScript
static/              Files served as they are
```

Import from `src/lib` through the `#lib/` alias and include the file extension, for
example `#lib/swarm/swarm.ts`.

## Design system

[`tokens.css`](src/lib/styles/tokens.css) is the single source of design values, as
CSS custom properties in two tiers: a palette of raw values, and roles that refer to
the palette. Components use roles only.

- **Color**: `--color-bg`, `--color-text` and its `-muted`, `-subtle` and `-faint`
  steps, `--color-border`, `--color-accent`. The formation editor adds
  `--color-panel`, `--color-surface` and `--color-surface-hover` for its layers. The
  `--swarm-*` roles color the canvas.
- **Type**: three families, `--font-mono` (Martian Mono) for text, `--font-display`
  (Major Mono Display) for the headline and `--font-sans` (Geist) for the formation
  editor's interface. Each text style is a `font` shorthand (`--text-display`,
  `--text-body`, `--text-code`, `--text-label`, and `--text-ui`, `--text-ui-heading`
  and `--text-ui-small` in the editor) with a matching `--tracking-*` where the style
  needs one. In the editor, mono is kept for identifiers and code: role names and the
  prompt.
- **Space**: `--space-N`, where N is the size in pixels at the default root size.
- **Layout**: `--gutter-inline`, `--gutter-block-end`, `--measure-display`,
  `--measure-body`, `--border-hairline`. The formation editor rounds its corners with
  `--radius-panel` and `--radius-control`, and lifts menus with `--shadow-popover`.
- **Text over the swarm**: `--text-halo`, a `text-shadow` in the background color
  that dims the swarm right around the letterforms so text stays readable as it
  passes behind. Apply it to any text placed over the canvas.

[`fonts.css`](src/lib/styles/fonts.css) declares the three self-hosted font faces, and
[`base.css`](src/lib/styles/base.css) holds element defaults (links, focus ring,
selection). Everything else is scoped to the component that uses it. Sizes are in
`rem`, so the page follows the reader's text size.

Add a token only when something uses it, and add a palette value before a role that
needs it.

## First-contact guide

`/start` follows the [first-contact contract](../../docs/first-contact.md). Its
entry prompt, routing questions and harness routes live in
[`guide.ts`](src/lib/onboarding/guide.ts); change them together with the contract.
A test checks that the prompt matches the contract word for word.

- The prompt only points the agent at `https://locust.farm/start`. It carries no
  command, download, invitation or secret. The address is the canonical one; the
  preview is hosted at that address behind Basic Auth.
- `SETUP_ARTIFACT` is unset because no setup is published. The guide then tells
  agents to report their harness and capabilities and stop. Set it to lane B's
  canonical setup location once that exists, and update each route's status from
  lane B's qualification records only.
- The copy button reports success only when the clipboard write succeeds. On
  failure it says so in a live status and selects the prompt for manual copying.
  Without JavaScript the button is not rendered and the prompt stays selectable.
- Harness details are native `<details>` elements, so they work without JavaScript.
- `/llms.txt` is a prerendered route built by [`llms.ts`](src/lib/onboarding/llms.ts)
  from the same content: the site summary, its pages, the entry prompt, and the
  instructions for agents and harness routes as plain Markdown. The agent steps live
  in `guide.ts` so the page and the text file cannot drift apart.

## Development manual

Canonical prose lives in `docs/guide/` at the repository root. `docs/site.json`
explicitly selects pages and metadata. The build module `src/lib/docs/content.ts`
renders Markdown with raw HTML disabled, assigns deterministic heading IDs,
validates metadata and links, and maps selected article links to public routes.
Explicitly approved engineering/source links map to GitHub at the source commit. Dirty source
trees are labeled explicitly; production artifacts should use committed source.

Each `/docs/next/<slug>` page is prerendered with raw Markdown under
`/docs/next/raw/<slug>.md`. `/docs/next/index.json` records the source commit,
contract versions, status and SHA-256 hashes of the served raw Markdown. Search
loads that inventory on first use. Navigation and reading work without JavaScript.
Unknown slugs return 404 and never resolve arbitrary files. Generated schema/contract/examples are exact downloadable assets selected in the
manifest; reference tables derive from those exports, with parity tests. The shared
`docs/reference/availability.json` supplies public-install facts to `/start`,
`llms.txt` and the manual. Code-copy controls reuse the truthful clipboard helper
and select the code for manual copying on failure. No released track or
public installation availability is claimed.

Use Node 22.18 or newer (Node 24.14.1 is pinned by .node-version and CI). CI should run `npm ci`,
`npm run lint`, `npm run check`, `npm test` and `npm run build` in this directory.
Prose changes require no Rust build; contract export/parity checks belong to Rust.

## Deployment

The preview runs on the shared 3cf.ai Nginx host, `root@96.126.103.38`.
[`adapter-static`](https://svelte.dev/docs/kit/adapter-static) emits the complete site
under `build/`; the postbuild check validates the published pages, links, client
assets and documentation hashes. No Node process is required on the server.

Use the Node version in `.node-version` and run:

```sh
bash scripts/deploy-production.sh
```

The script builds committed `HEAD` in a temporary clean checkout, runs all four
site gates, uploads a release to `/var/www/locust.farm/releases/<commit>`, verifies
its SHA-256 inventory on the server, and atomically switches `current`. Uncommitted
work is excluded. It requires the initial server provisioning below and checks
that public requests receive HTTP 401. Authenticated content and browser behavior
must also be verified after each deployment. Earlier releases remain available
for rollback by changing the `current` symlink.

[`ops/nginx.conf`](ops/nginx.conf) serves both `locust.farm` and `www.locust.farm`,
redirects HTTP to HTTPS, maps extensionless routes to prerendered HTML and returns
404 for unknown routes. Basic Auth applies to all HTTPS pages and assets, with
`private, no-store` and `noindex, nofollow` response headers. Only HTTP ACME
challenges are public, from `/var/www/letsencrypt`.

Initial provisioning uses an HTTP-only virtual host exposing that challenge
directory and returning 401 elsewhere. Issue the certificate with Certbot's
`certonly --webroot -w /var/www/letsencrypt -d locust.farm -d www.locust.farm` mode,
using the server's existing ACME account. Install `ops/nginx.conf` as
`/etc/nginx/sites-available/locust.farm`, link it in `sites-enabled`, test with
`nginx -t`, and reload. The certificate and full trust chain live at
`/etc/letsencrypt/live/locust.farm/`; renewal uses webroot validation and a deploy
hook to reload Nginx. Keep the existing server renewal scheduler enabled.

The password file is `/etc/nginx/locust.farm.htpasswd`, owned by `root:www-data`
with mode `0640`. Provision it using `htpasswd -cB` with an interactive password
prompt or stdin; keep credentials and password hashes off Git. DNS needs an apex
A record for `96.126.103.38` and a `www` CNAME to `locust.farm`.

## Formation editor

`/formations` lets a person who has Locust build a formation and copy one prompt
that has their coding agent add it to their Locust. The plan is in
[`docs/formation-authoring-plan.md`](../../docs/formation-authoring-plan.md) and the
prompt's fixed text in [`docs/formation-prompt.md`](../../docs/formation-prompt.md).

- The page is prerendered with its heading and explanation; the editor loads after
  hydration, so its code stays out of every other page.
- `contract/` ports Locust's offline checks to TypeScript. A test runs them on
  every conformance vector that `scripts/check_formations.py` generates with the
  real CLI and requires the same codes, paths, messages and explanations.
- `model/` holds the document and every edit, so references stay consistent when
  roles, steps or kinds of task are renamed or removed. `model/line.ts` reads the
  four answers of a line (who adds a task, who works on it, when a result counts,
  whether one result is picked) from the formation and writes them back.
  `prompt/` builds the prompt and reads pasted prompts, replies and JSON back.
  `storage/` keeps formations in the browser and makes share links.
- `ui/` draws the page: the six cards, the line of four points with a picture
  at each (`diagrams.ts`), the box of choices under a point, the rows for steps
  and other kinds of task, and one side panel. The side panel is adapted from
  Polaris's settings panel (dreamcolor10 at `01d8aa3c4`,
  `crates/polaris/frontend/src/lib/components/blueprint/InspectorPanel.svelte`)
  and restyled with this site's tokens. `ui/icons.ts` holds the Phosphor icons
  the page shows, from `@phosphor-icons/core` 2.1.1. There is no canvas.
- `e2e/formations.spec.ts` drives the built page in Chromium: the first visit,
  the four points and their pictures, the card that matches the rules, roles made
  in place, steps, other kinds of task, problems, share links, a phone viewport
  and the page without JavaScript. When `target/debug/locust` exists it also checks that
  the copied prompt's formation passes `locust formation validate`. Install the
  browser once with `npx playwright install chromium`.

## Swarm animation

`SwarmCanvas.svelte` runs [`createSwarm`](src/lib/swarm/swarm.ts) on a canvas. The
agents and the locust's cells are simulated on the GPU with transform feedback
([`renderer.ts`](src/lib/swarm/renderer.ts), [`shaders.ts`](src/lib/swarm/shaders.ts)),
so a step costs the main thread a fixed handful of draw calls, whatever the number of
agents. The locust's shape and its Game of Life are plain data on the CPU
([`locust.ts`](src/lib/swarm/locust.ts)) and are covered by unit tests.

- The simulation advances at a fixed 120 steps per second, independent of the
  display's refresh rate. On a 60 Hz display that is two steps per frame, drawn
  under a single fade of the trail buffer, which is the costly part of a step.
- The guide draws 700 wandering agents behind its top section, faded out before
  the details.
- The homepage uses `goalMode="cursor"`: the swarm follows the pointer, and
  wanders on its own when there is none. `catchUp` sets how much harder agents far
  from the cursor are pulled toward it; 0 gives the design's original, slower chase.
- Colors come from the `--swarm-*` tokens, read once when the canvas mounts.
- The drawing buffer uses a pixel ratio of at most 2 and is no larger than a 4K
  screen; bigger canvases are drawn at a lower ratio.
- With `prefers-reduced-motion`, a single still frame is drawn instead.
- Without WebGL2, or where it would run in software, the canvas stays empty and the
  page is otherwise unaffected.
