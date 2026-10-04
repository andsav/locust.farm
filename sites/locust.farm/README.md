# locust.farm

The Locust marketing site: three prerendered SvelteKit pages over a WebGL2 swarm
animation. `/` says what Locust is for, `/start` is the first-contact guide with
the entry prompt, and `/docs` is a placeholder for future documentation. It is a
self-contained npm project with its own `package.json` and `node_modules`.

```sh
npm install
npm run dev      # local dev server
npm run check    # type checking
npm run lint     # Prettier and ESLint
npm test         # unit tests, using Node's built-in runner
npm run build
```

Tests need Node 22.18 or newer, which runs TypeScript directly. After installing or
removing a package, run `npx svelte-kit sync` (or any `npm run` script that does):
npm prunes the generated `node_modules/$app` directory that `tsconfig.json` extends.

## Layout

```text
src/routes/          The homepage, the /start guide, the /docs placeholder, and the root layout that loads fonts and global styles
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
  steps, `--color-border`, `--color-accent`. The `--swarm-*` roles color the canvas.
- **Type**: two families, `--font-mono` (Martian Mono) for text and `--font-display`
  (Major Mono Display) for the headline. Each text style is a `font` shorthand
  (`--text-display`, `--text-body`, `--text-code`, `--text-label`) with a matching
  `--tracking-*` where the style needs one.
- **Space**: `--space-N`, where N is the size in pixels at the default root size.
- **Layout**: `--gutter-inline`, `--gutter-block-end`, `--measure-display`,
  `--measure-body`, `--border-hairline`.
- **Text over the swarm**: `--text-halo`, a `text-shadow` in the background color
  that dims the swarm right around the letterforms so text stays readable as it
  passes behind. Apply it to any text placed over the canvas.

[`fonts.css`](src/lib/styles/fonts.css) declares the two self-hosted font faces, and
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
  page is available locally and is not deployed by this repository.
- `SETUP_ARTIFACT` is unset because no setup is published. The guide then tells
  agents to report their harness and capabilities and stop. Set it to lane B's
  canonical setup location once that exists, and update each route's status from
  lane B's qualification records only.
- The copy button reports success only when the clipboard write succeeds. On
  failure it says so in a live status and selects the prompt for manual copying.
  Without JavaScript the button is not rendered and the prompt stays selectable.
- Harness details are native `<details>` elements, so they work without JavaScript.

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
