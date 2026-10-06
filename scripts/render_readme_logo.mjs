#!/usr/bin/env node
// Rebuild the README artwork with the website's actual WebGL renderer.
// Requires: npm ci in sites/locust.farm, Playwright Chromium, and ffmpeg on PATH.
// From the repository root: node scripts/render_readme_logo.mjs
import { createRequire } from 'node:module';
import { mkdir, mkdtemp, readFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';
import { execFileSync } from 'node:child_process';

const root = fileURLToPath(new URL('../', import.meta.url));
const require = createRequire(join(root, 'sites/locust.farm/package.json'));
const { chromium } = require('playwright');
const ts = require('typescript');
const source = join(root, 'sites/locust.farm/src/lib');
await mkdir(join(root, 'output/playwright'), { recursive: true });
const scratch = await mkdtemp(join(root, 'output/playwright/readme-logo-'));
const assets = join(root, 'docs/assets');
await mkdir(assets, { recursive: true });

const modules = new Map();
for (const name of ['locust', 'renderer', 'gl', 'shaders']) {
  const input = await readFile(join(source, `swarm/${name}.ts`), 'utf8');
  modules.set(`/${name}.js`, ts.transpileModule(input, {
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 },
  }).outputText.replaceAll('.ts\'', '.js\''));
}
const css = await readFile(join(source, 'styles/tokens.css'), 'utf8');
modules.set('/', `<!doctype html><meta charset="utf-8"><title>README logo capture</title>
<style>${css} body { margin: 0; background: var(--swarm-bg); } canvas { display: block; }</style>
<canvas width="1440" height="900"></canvas><script type="module">
import { createLocust } from './locust.js';
import { SwarmRenderer } from './renderer.js';
// Fixed seed makes the initial composition repeatable on the same GPU/browser.
let seed = 731;
Math.random = () => ((seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0) / 4294967296);
const canvas = document.querySelector('canvas');
const gl = canvas.getContext('webgl2', { alpha: false, antialias: false, preserveDrawingBuffer: true });
if (!gl) throw new Error('WebGL2 is required');
function color(name) {
  const hex = getComputedStyle(canvas).getPropertyValue(name).trim().slice(1);
  return [0, 2, 4].map(i => parseInt(hex.slice(i, i + 2), 16) / 255);
}
const locust = createLocust();
const renderer = new SwarmRenderer(gl, locust, {
  background: color('--swarm-bg'), agent: color('--swarm-agent'), goal: color('--swarm-goal'),
  cellBody: color('--swarm-cell-body'), cellLeg: color('--swarm-cell-leg'), cellEye: color('--swarm-cell-eye'),
});
renderer.resize(1440, 900, 1);
renderer.setAgents(1400);
// Keep the silhouette calm, as in the reference; only the swarm moves.
renderer.setAlive(new Uint8Array(locust.count));
window.advance = (steps) => {
  for (let i = 0; i < steps; i++) {
    renderer.simulate(370, 360, 0);
    renderer.fade(0.28);
    renderer.draw(370, 360);
  }
  renderer.present();
};
window.advance(1800);
window.ready = true;
</script>`);

const server = createServer((request, response) => {
  const body = modules.get(request.url);
  response.writeHead(body ? 200 : 404, {
    'Content-Type': request.url === '/' ? 'text/html' : 'text/javascript',
  });
  response.end(body ?? 'Not found');
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
let browser;
try {
  // Headed Chromium uses the native GPU; the headless shell can fall back to
  // software WebGL, making this capture needlessly expensive.
  browser = await chromium.launch({ headless: false });
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  page.on('pageerror', error => console.error(error));
  await page.goto(`http://127.0.0.1:${server.address().port}`);
  await page.waitForFunction(() => window.ready);
  for (let frame = 0; frame < 160; frame++) {
    // Half of the site's usual speed for ambient motion at 20 fps.
    await page.evaluate(() => window.advance(3));
    await page.screenshot({
      path: join(scratch, `${String(frame).padStart(3, '0')}.png`),
      clip: { x: 170, y: 165, width: 1220, height: 470 },
    });
    if (frame % 40 === 0) console.log(`Captured ${frame + 1}/160 frames`);
  }
} finally {
  await browser?.close();
  server.close();
}

// Blend the last second into the first second, then begin playback at second one.
// The wrap therefore advances naturally from source frame 19 to source frame 20.
const filter = [
  '[0:v]split=3[head][middle][tail]',
  '[head]trim=end_frame=20,setpts=PTS-STARTPTS[h]',
  '[middle]trim=start_frame=20:end_frame=140,setpts=PTS-STARTPTS[m]',
  '[tail]trim=start_frame=140:end_frame=160,setpts=PTS-STARTPTS[t]',
  "[t][h]blend=all_expr='A*(1-T)+B*T'[seam]",
  '[m][seam]concat=n=2:v=1:a=0,scale=976:376:flags=lanczos,split[a][b]',
  '[a]palettegen=max_colors=128:reserve_transparent=0[p]',
  '[b][p]paletteuse=dither=none',
].join(';');
execFileSync('ffmpeg', ['-hide_banner', '-loglevel', 'error', '-y',
  '-framerate', '20', '-i', join(scratch, '%03d.png'),
  '-filter_complex', filter, '-loop', '0', join(assets, 'locust-swarm.gif')], { stdio: 'inherit' });
console.log(`Saved ${join(assets, 'locust-swarm.gif')}`);
