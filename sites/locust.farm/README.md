# locust.farm

The Locust website, built with SvelteKit and prerendered to static files. It has these pages:

- `/`: what Locust is for, over the swarm animation.
- `/start`: the setup prompt.
- `/formations`: the formation editor.
- `/docs`: the development manual, built from `docs/guide/`.
- `/farm/<id>` and `/farms`: public farm pages.

It is a self-contained npm project.

```sh
npm ci
npm run dev      # local dev server
npm run check    # type checking
npm run lint     # Prettier and ESLint
npm test         # unit tests, using Node's built-in runner
npm run build    # production build, then scripts/check-prerender.mjs
npm run test:e2e # Playwright tests in Chromium, against a production build
```

Use Node 22.18 or newer, which runs TypeScript directly. `.node-version` and CI use Node 24.14.1.
After installing or removing a package, run `npx svelte-kit sync` (or any `npm run` script that
does): npm prunes the generated `node_modules/$app` directory that `tsconfig.json` extends.

CI ([site.yml](../../.github/workflows/site.yml)) runs `npm ci`, `npm run lint`, `npm run check`,
`npm test` and `npm run build` for pushes to `main` and pull requests. It does not run the
Playwright tests. Install their browser once with `npx playwright install chromium`.

## Layout

```text
src/routes/                Pages, the docs routes, llms.txt, sitemap.xml and the root layout
src/lib/components/        Shared Svelte components
src/lib/docs/              The manual build: reads docs/site.json and renders docs/guide/
src/lib/farm/              Farm views, stage map, live client and generated types
src/lib/formation-editor/  The formation editor
src/lib/onboarding/        The /start content, the llms.txt text and the clipboard helper
src/lib/styles/            Design tokens, element defaults and shared components
src/lib/swarm/             The swarm animation
src/lib/site.ts            Header links
e2e/                       Playwright tests for /formations and the farm pages
scripts/                   Build check, deployment script and farm type generator
ops/                       Nginx config, farm service unit and the farm operator guide
static/                    Files served as they are
```

Import from `src/lib` through the `#lib/` alias and include the file extension, for example
`#lib/swarm/swarm.ts`.

## Design system

The root layout loads the design system from [`src/lib/styles/`](src/lib/styles/). Use it before
writing new styles.

- [`tokens.css`](src/lib/styles/tokens.css): every design value, as a palette and roles that refer
  to it. Styles use roles only.
- [`base.css`](src/lib/styles/base.css): element defaults, such as body text, headings, code, links
  and the focus ring.
- [`components.css`](src/lib/styles/components.css): shared classes (table below).
- [`prose.css`](src/lib/styles/prose.css): Markdown rendered under `.prose`.
- [`fonts.css`](src/lib/styles/fonts.css): Geist for text, Major Mono Display for headlines, and
  Martian Mono for code, identifiers and labels.

Token names: `--color-*` (with `--swarm-*` for the canvas), `--text-*` font shorthands,
`--space-N` (N is pixels at the default root size), layout (`--gutter-*`, `--measure-*`,
`--border-hairline`), `--radius-*`, `--shadow-popover` and `--duration-*`. Apply `--text-halo` to
any text over the swarm canvas so it stays readable.

| Class                        | Use                                                                  |
| ---------------------------- | -------------------------------------------------------------------- |
| `.button`                    | A button, or a link that acts as one.                                |
| `.button.primary`            | The one main action of a page or panel.                              |
| `.button.quiet`              | An action beside a stronger one, or in text.                         |
| `.icon-button`               | A square button that holds one icon.                                 |
| `.input`, `.field`, `.label` | A form field, and a label above it.                                  |
| `.card`                      | A raised block that groups content; as a link, it has hover.         |
| `.well`                      | Text to read exactly or copy: a prompt or a block of code.           |
| `.eyebrow`                   | A short uppercase label above a title.                               |
| `.accent`                    | The accent color on a word or mark.                                  |
| `.skip-link`                 | The link that jumps past navigation on a long page.                  |
| `.prose`                     | Long-form text rendered from Markdown.                               |
| `.ui`                        | On a container: bare buttons and fields inside take the shared look. |

Sizes are in `rem`. A component's scoped styles override a shared class. Add a token only when
something uses it.

## Setup prompt

`/start` shows the setup prompt from [first-contact.md](../../docs/first-contact.md). The prompt
tells the agent to install or update Locust from the public files at `https://locust.farm/downloads/`,
start the daemon and connect itself. The agent never needs the password-protected page. Below
it are what happens next, a note on Polaris, the agent's steps, setup routes for Claude Code,
Codex, pi, Droid and other agents, and a portable CLI setup.

- The text lives in [`guide.ts`](src/lib/onboarding/guide.ts). A test checks that the prompt
  matches first-contact.md word for word.
- [`llms.ts`](src/lib/onboarding/llms.ts) builds `/llms.txt` from the same data and
  `docs/reference/availability.json`.
- The copy button reports success only when the clipboard write succeeds; otherwise it selects the
  prompt. The page works without JavaScript.

## Development manual

`/docs` serves the manual in `docs/guide/`, and `docs/site.json` chooses its pages.
[docs/manual.md](../../docs/manual.md) explains how the manual is built and checked.

- `src/lib/docs/content.ts` reads the repository at build time, so the build needs a Git checkout.
- There is one development track, under `/docs/next/`. It describes the current source; the public
  macOS preview at `/downloads/` can be older.
- Links to repository files outside `docs/guide/` become GitHub links at the source commit. The
  repository is private, so only collaborators can open them.

## Deployment

The preview runs on the shared 3cf.ai Nginx host, `root@96.126.103.38`.
[`adapter-static`](https://svelte.dev/docs/kit/adapter-static) writes the whole site to `build/`.
No Node process runs on the server. Deploy with the Node version in `.node-version`:

```sh
bash scripts/deploy-production.sh
```

The script builds committed `HEAD` in a clean temporary checkout and runs lint, check, test and
build. It uploads the build to `/var/www/locust.farm/releases/<commit>`, checks its SHA-256 list on
the server, switches the `current` symlink and checks that preview pages answer 401 without the
password. Then check the pages yourself in a browser. To roll back, point `current` at an earlier
release.

[`ops/nginx.conf`](ops/nginx.conf) serves `locust.farm` and `www.locust.farm`:

- HTTP redirects to HTTPS, except ACME challenges from `/var/www/letsencrypt`.
- Unknown routes return 404, except `/farm/<id>`, which gets the `farm.html` shell.
- Preview pages need Basic Auth and send `private, no-store` and `noindex, nofollow`.
- Farm pages, the gallery, their client assets and `/api/farms` are public. The API proxies to the
  separate `locust-farm` process; the [farm operator guide](ops/README.md) covers it.
- `/downloads/` is public and serves release files from `/var/www/locust.farm/downloads/`, outside
  `current`, with no directory listing. Website deployment and rollback do not touch it.

First-time setup:

1. Serve an HTTP-only virtual host that exposes the challenge directory and returns 401 elsewhere.
2. Issue the certificate with Certbot,
   `certonly --webroot -w /var/www/letsencrypt -d locust.farm -d www.locust.farm`, using the
   server's existing ACME account. Renewal uses the same webroot and a deploy hook that reloads
   Nginx.
3. Install `ops/nginx.conf` as `/etc/nginx/sites-available/locust.farm`, link it in
   `sites-enabled`, run `nginx -t` and reload.
4. Create `/etc/nginx/locust.farm.htpasswd` with `htpasswd -cB` (owner `root:www-data`, mode
   `0640`). Keep passwords and hashes out of Git.
5. DNS: an apex A record for `96.126.103.38` and a `www` CNAME to `locust.farm`.

## Live farms

`/farm/<id>` loads a farm's public snapshot, then follows full updates over server-sent events.
`/farms` shows the listed, available farms. Both use the same stage map, with a task table for
keyboard and screen readers. All data comes from the API.

For local development, run `cargo run -p locust-farm -- serve --database /PATH/TO/farm.sqlite` (it
listens on `127.0.0.1:4319`), then `npm run dev`. `LOCUST_FARM_API` changes the proxy target. The
static adapter writes `farm.html` as the shell for farm URLs.

After `cargo build --locked -p locust`, `python3 ../../scripts/check_formations.py --write` exports
the Rust `FarmSnapshot` schema to `docs/reference/generated/farm.schema.json`. Then run
`node scripts/generate-farm-types.mjs --write` here; a unit test fails when the generated types are
stale. The Rust service validates every snapshot it saves. The owner commands and consent
steps are in [Public farm views](../../docs/guide/farm-publication.md).

## Formation editor

`/formations` lets a person build a formation and copy one prompt that has their coding agent add
it to their Locust. [docs/formation-editor.md](../../docs/formation-editor.md) describes the
editor, and [docs/formation-prompt.md](../../docs/formation-prompt.md) holds the prompt's fixed
text.

- The editor loads after hydration, so its code stays out of every other page.
- `contract/` ports Locust's offline checks to TypeScript. A test requires the same results as the
  real CLI on every conformance vector from `scripts/check_formations.py`.
- `model/` holds the document and every edit. `model/line.ts` reads and writes a row's four
  answers: who adds a task, who works on it, when a result counts, and whether one is picked.
- `prompt/` builds the prompt and reads pasted prompts, replies and JSON. `storage/` saves
  formations in the browser and makes share links.
- `ui/` draws the page. Six cards show the ways of working, each with a picture (`diagrams.ts`). A
  compact rules matrix (`Lines.svelte`) has one row for any task and one per step or task type;
  each cell opens its choices (`PointBox.svelte`). There is no canvas.
- The side panel (roles, problems, the prompt, saved formations) is adapted from Polaris's settings
  panel and restyled with this site's tokens; `SidePanel.svelte` names its source. `ui/icons.ts`
  holds the Phosphor icons the page uses, from `@phosphor-icons/core` 2.1.1.
- `e2e/formations.spec.ts` drives the built page. When `target/debug/locust` exists, it also checks
  that the copied formation passes `locust formation validate`.

## Swarm animation

`SwarmCanvas.svelte` runs [`createSwarm`](src/lib/swarm/swarm.ts) on a canvas, on the homepage
only. The GPU simulates the agents and the locust's cells with transform feedback
([`renderer.ts`](src/lib/swarm/renderer.ts), [`shaders.ts`](src/lib/swarm/shaders.ts)), so the main
thread's cost does not grow with the number of agents. The locust's shape and its Game of Life are
plain data on the CPU ([`locust.ts`](src/lib/swarm/locust.ts)), covered by unit tests.

- The simulation runs at a fixed 120 steps per second, whatever the display's refresh rate.
- With `goalMode="cursor"` the swarm follows the pointer. `catchUp` sets how much harder far agents
  are pulled toward it; 0 gives the original, slower chase.
- The drawing buffer uses a pixel ratio of at most 2 and is no larger than a 4K screen.
- With `prefers-reduced-motion`, one still frame is drawn. Without hardware WebGL2 the canvas stays
  empty.
