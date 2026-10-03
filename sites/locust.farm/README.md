# locust.farm

The Locust marketing site: a prerendered SvelteKit page with a WebGL2 swarm
animation. It is a self-contained npm project with its own `package.json` and
`node_modules`.

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
src/routes/          The landing page, and the root layout that loads fonts and global styles
src/lib/styles/      Design tokens and element defaults
src/lib/components/  Svelte components
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
- **Type**: one family, `--font-mono`. Each text style is a `font` shorthand
  (`--text-display`, `--text-body`, `--text-code`, `--text-label`) with a matching
  `--tracking-*` where the style needs one.
- **Space**: `--space-N`, where N is the size in pixels at the default root size.
- **Layout**: `--gutter-inline`, `--gutter-block-end`, `--measure-display`,
  `--measure-body`, `--border-hairline`.

[`base.css`](src/lib/styles/base.css) holds element defaults (links, focus ring,
selection). Everything else is scoped to the component that uses it. Sizes are in
`rem`, so the page follows the reader's text size.

Add a token only when something uses it, and add a palette value before a role that
needs it.

## Swarm animation

`SwarmCanvas.svelte` attaches [`swarm`](src/lib/swarm/swarm.ts) to a canvas. The
agents and the locust's cells are simulated on the GPU with transform feedback
([`renderer.ts`](src/lib/swarm/renderer.ts), [`shaders.ts`](src/lib/swarm/shaders.ts)),
so a step costs the main thread a fixed handful of draw calls, whatever the number of
agents. The locust's shape and its Game of Life are plain data on the CPU
([`locust.ts`](src/lib/swarm/locust.ts)) and are covered by unit tests.

- The simulation advances at a fixed 120 steps per second, independent of the
  display's refresh rate.
- Colors come from the `--swarm-*` tokens, read once when the canvas mounts.
- With `prefers-reduced-motion`, a single still frame is drawn instead.
- Without WebGL2 the canvas stays empty and the page is otherwise unaffected.
