import { execFileSync } from 'node:child_process';
import { existsSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { expect, test, type Page } from '@playwright/test';

const locust = new URL('../../../target/debug/locust', import.meta.url).pathname;

async function open(page: Page) {
	await page.goto('/blueprints');
	await page.evaluate(() => localStorage.clear());
	await page.reload();
	await expect(page.getByRole('button', { name: 'Copy prompt' })).toBeEnabled();
}

/** Opens one of the side panels from the toolbar. */
async function panel(page: Page, name: string) {
	await page.getByRole('button', { name, exact: true }).click();
}

/** The lines between a block's BEGIN and END lines. */
function block(prompt: string, name: string): string {
	const lines = prompt.split('\n');
	const begin = lines.findIndex((line) => line.startsWith(`BEGIN LOCUST ${name}`));
	const end = lines.findIndex((line) => line.startsWith(`END LOCUST ${name}`));
	return lines.slice(begin + 1, end).join('\n');
}

test('a first visit is ready to copy with the Open way of working', async ({ page }) => {
	await open(page);
	await expect(page.getByRole('heading', { level: 1 })).toHaveText('How does your team work?');
	await expect(page.locator('.ways .way')).toHaveCount(6);
	await expect(page.locator('.ways .way input[value="open"]')).toBeChecked();
	await expect(page.getByRole('button', { name: 'No problems found' })).toBeVisible();
	await panel(page, 'In words');
	await expect(page.locator('.summary .lines')).toContainText(
		'A task is done when the person who did it says so.'
	);
	await panel(page, 'See the prompt');
	await expect(page.locator('pre.prompt')).toContainText(
		'Please add this Locust blueprint to my local Locust as a private draft.'
	);
});

test('the page shows pictures and icons, with the words in tooltips', async ({ page }) => {
	await open(page);
	await expect(page.locator('.map .picture')).toBeVisible();
	await page.getByRole('button', { name: 'Copy link' }).hover();
	await expect(page.locator('.bp-tip')).toContainText('Copy link');
	await page.locator('.ways .way', { hasText: 'Open' }).hover();
	await expect(page.locator('.bp-tip')).toContainText('Everyone works freely');
});

test('the picture on the map follows the rules', async ({ page }) => {
	await open(page);
	const drawn = () => page.locator('.map .picture').innerHTML();
	const before = await drawn();
	await page.getByRole('radio', { name: 'Review', exact: true }).click();
	await expect.poll(drawn).not.toBe(before);
	await expect(page.locator('.map .picture')).toContainText('never reviewed by its own author');
});

test('copying the prompt gives a blueprint the real Locust CLI accepts', async ({ page }) => {
	test.skip(!existsSync(locust), 'no local Locust build (cargo build -p locust)');
	await open(page);
	await page.locator('.ways .way', { hasText: 'Pipeline' }).click();
	await page.getByRole('button', { name: 'Copy prompt' }).click();
	await expect(page.getByText('Copied. Paste it into your agent.')).toBeVisible();
	const prompt = await page.evaluate(() => navigator.clipboard.readText());
	const file = join(mkdtempSync(join(tmpdir(), 'locust-e2e-')), 'blueprint.json');
	writeFileSync(file, block(prompt, 'BLUEPRINT'));
	const result = JSON.parse(
		execFileSync(locust, ['--json', 'blueprint', 'validate', file], { encoding: 'utf8' })
	);
	expect(result.ok).toBe(true);
	expect(result.result.valid).toBe(true);
	expect(Object.keys(result.result.normalized.flow)).toEqual(['draft', 'review']);
});

test('the pipeline shows its stages, arrow and list', async ({ page }) => {
	await open(page);
	await page.locator('.ways .way', { hasText: 'Pipeline' }).click();
	await expect(page.locator('.svelte-flow__node')).toHaveCount(2);
	await expect(page.locator('.stage-edge-label')).toHaveText('when complete');
	await expect(page.locator('.map .picture')).toHaveCount(0);
	await panel(page, 'In words');
	await expect(page.locator('.summary .lines')).toContainText(
		'Stage "review" starts when "draft" is complete.'
	);
});

test('dropping a connection on empty map adds a stage that waits for it', async ({ page }) => {
	await open(page);
	await page.locator('.ways .way', { hasText: 'Pipeline' }).click();
	const port = page.locator('.svelte-flow__node[data-id="review"] .svelte-flow__handle-right');
	const box = (await port.boundingBox())!;
	await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
	await page.mouse.down();
	await page.mouse.move(box.x + 60, box.y + 160, { steps: 8 });
	await page.mouse.up();
	await expect(page.locator('.svelte-flow__node')).toHaveCount(3);
	await expect(page.locator('.panel.open h2')).toHaveText('step');
	await expect(page.locator('.panel.open select[aria-label="Stage it waits for"]')).toHaveValue(
		'review'
	);
	const draft = await page.locator('.svelte-flow__node[data-id="draft"]').boundingBox();
	await page.getByRole('button', { name: 'Undo' }).click();
	await expect(page.locator('.svelte-flow__node')).toHaveCount(2);
	expect(draft).not.toBeNull();
});

test('stages can be added, connected by keyboard, and undone', async ({ page }) => {
	await open(page);
	await page.getByRole('button', { name: 'Add a stage' }).click();
	await expect(page.locator('.panel.open h2')).toHaveText('step');
	await page.getByRole('button', { name: 'Close' }).click();
	await page.getByRole('button', { name: 'Add a stage' }).first().click();
	await page.getByRole('button', { name: 'Close' }).click();
	await expect(page.locator('.svelte-flow__node')).toHaveCount(2);

	await page.locator('.svelte-flow__node[data-id="step"]').focus();
	await page.keyboard.press('c');
	const dialog = page.getByRole('dialog', { name: 'Connect stages' });
	await expect(dialog).toContainText('Start "step 2" when "step"');
	await dialog.getByRole('button', { name: 'is complete' }).click();
	await expect(page.locator('.svelte-flow__edge')).toHaveCount(1);
	await panel(page, 'In words');
	await expect(page.locator('.summary .lines')).toContainText(
		'Stage "step 2" starts when "step" is complete.'
	);
	await page.getByRole('button', { name: 'Close' }).click();

	await page.getByRole('button', { name: 'Undo' }).click();
	await expect(page.locator('.svelte-flow__edge')).toHaveCount(0);
	await page.getByRole('button', { name: 'Redo' }).click();
	await expect(page.locator('.svelte-flow__edge')).toHaveCount(1);
});

test('a connection that would loop is refused with a reason', async ({ page }) => {
	await open(page);
	await page.locator('.ways .way', { hasText: 'Pipeline' }).click();
	await page.locator('.svelte-flow__node[data-id="review"]').focus();
	await page.keyboard.press('c');
	const dialog = page.getByRole('dialog', { name: 'Connect stages' });
	await dialog.getByRole('combobox').selectOption('draft');
	await dialog.getByRole('button', { name: 'is complete' }).click();
	await expect(page.getByText('That would make "review" wait for itself.').first()).toBeVisible();
	await expect(page.locator('.svelte-flow__edge')).toHaveCount(1);
});

test('problems are explained in plain words and can be found', async ({ page }) => {
	await open(page);
	await page.locator('.ways .way', { hasText: 'Coordinator' }).click();
	await page.getByRole('button', { name: 'Role "coordinator"' }).click();
	await page.getByRole('button', { name: 'Remove role' }).click();
	await page.getByRole('button', { name: /^\d+ problems?$/ }).click();
	await expect(page.locator('.panel.open h2')).toHaveText(/things? to check/);
	await expect(page.locator('.problems')).toContainText('Nobody could ever finish a task');
	await expect(page.getByRole('radio', { name: 'Add', exact: true })).toHaveAttribute(
		'aria-description',
		/unfinished private draft/
	);
	await page.locator('.problems').getByRole('button', { name: 'Show me' }).first().click();
	await expect(page.locator('#section-done')).toBeInViewport();
	await page.getByRole('button', { name: 'Undo' }).click();
	await expect(page.getByRole('button', { name: 'No problems found' })).toBeVisible();
});

test('a share link opens the same blueprint as a new copy', async ({ page, context }) => {
	await open(page);
	await page.locator('.ways .way', { hasText: 'Pipeline' }).click();
	await page.getByLabel('Name of this blueprint').fill('Sync research');
	await page.getByLabel('Name of this blueprint').press('Enter');
	await page.getByRole('button', { name: 'Copy link' }).click();
	await expect(page.locator('.toast')).toContainText('Link copied');
	const url = await page.evaluate(() => navigator.clipboard.readText());
	expect(url).toMatch(/\/blueprints#b1\./);
	const other = await context.newPage();
	await other.goto(url);
	await expect(other.getByText('Opened from a link.')).toBeVisible();
	await expect(other.locator('.svelte-flow__node')).toHaveCount(2);
	await expect(other.getByLabel('Name of this blueprint')).toHaveValue('Sync research');
	expect(other.url()).not.toContain('#');
});

test('on a phone the page fits and the map opens full screen', async ({ page }) => {
	await page.setViewportSize({ width: 375, height: 812 });
	await open(page);
	await page.locator('.ways .way', { hasText: 'Pipeline' }).click();
	const widths = await page.evaluate(() => [document.documentElement.scrollWidth, innerWidth]);
	expect(widths[0]).toBeLessThanOrEqual(widths[1]);
	await page.getByRole('button', { name: 'Open the map full screen' }).click();
	await expect(page.locator('.map.full-screen')).toBeVisible();
	await page.getByRole('button', { name: 'Close the map' }).click();
	await page.locator('.list').getByRole('button', { name: 'Edit' }).first().click();
	await expect(page.locator('.panel.open h2')).toHaveText('draft');
});

test.describe('without JavaScript', () => {
	test.use({ javaScriptEnabled: false });
	test('the page still explains blueprints and links to the guide', async ({ page }) => {
		await page.goto('/blueprints');
		await expect(page.getByRole('heading', { level: 1 })).toHaveText('How does your team work?');
		await expect(page.getByRole('link', { name: 'Not set up yet? Start here.' })).toBeVisible();
	});
});
