import { execFileSync } from 'node:child_process';
import { existsSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { expect, test, type Locator, type Page } from '@playwright/test';

const locust = new URL('../../../target/debug/locust', import.meta.url).pathname;

async function open(page: Page) {
	await page.goto('/blueprints');
	await page.evaluate(() => localStorage.clear());
	await page.reload();
	await expect(page.getByRole('button', { name: 'Copy prompt' })).toBeEnabled();
}

const way = (page: Page, title: string) => page.locator('.ways .way', { hasText: title });

/** One of the four points of the line for any task. */
const point = (page: Page, name: 'add' | 'work' | 'counts' | 'pick') =>
	page.locator(`.line.main [data-point="${name}"]`);

/** A step's or kind's row, by its name. */
const row = (page: Page, name: string): Locator => page.locator(`.line.row[data-name="${name}"]`);

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
	await expect(page.locator('.ways .diagram text')).toHaveCount(0);
	await expect(page.locator('.ways .title')).toHaveText([
		'Open',
		'Coordinator',
		'Peer review',
		'Review panel',
		'Independent attempts',
		'Pipeline'
	]);
	await expect(way(page, 'Open')).toHaveAttribute('aria-pressed', 'true');
	await expect(page.getByRole('button', { name: 'No problems' })).toBeVisible();
	await page.getByRole('button', { name: 'In words' }).click();
	await expect(page.locator('.summary .lines')).toContainText(
		'A result counts when its author says so.'
	);
	await page.getByRole('button', { name: 'See the prompt' }).click();
	await expect(page.locator('pre.prompt')).toContainText(
		'Please add this Locust blueprint to my local Locust as a private draft.'
	);
});

test('the four questions and their answers are on the page without hovering', async ({ page }) => {
	await open(page);
	await expect(point(page, 'add')).toContainText('Who adds tasks?');
	await expect(point(page, 'add')).toContainText('Anyone.');
	await expect(point(page, 'work')).toContainText('Who works on a task?');
	await expect(point(page, 'work')).toContainText(
		'No lock: two members can work on the same task.'
	);
	await expect(point(page, 'counts')).toContainText('When does a result count?');
	await expect(point(page, 'counts')).toContainText('When its author says so.');
	await expect(point(page, 'pick')).toContainText('Is one result picked?');
	await expect(point(page, 'pick')).toContainText('Nobody. Every result that counts stays.');
	await expect(page.locator('.notes')).toContainText('It does not start agents or run checks.');
});

test('each way of working is a different answer at one or two points', async ({ page }) => {
	await open(page);
	await way(page, 'Coordinator').click();
	await expect(point(page, 'work')).toContainText('"coordinator" asks a member. They can say no.');
	await expect(point(page, 'counts')).toContainText('After 1 approval from "coordinator".');
	await expect(point(page, 'pick')).toContainText('"coordinator" picks one result per task.');
	await way(page, 'Review panel').click();
	await expect(point(page, 'counts')).toContainText(
		'After 2 approvals from "reviewer", not the author\'s.'
	);
	await way(page, 'Independent attempts').click();
	await expect(point(page, 'counts')).toContainText('When its author says so.');
	await expect(point(page, 'pick')).toContainText('"judge" picks one result per task.');
});

test('the pictures are drawn from the rules, with their role names', async ({ page }) => {
	await open(page);
	const drawn = () => point(page, 'counts').locator('svg').innerHTML();
	const before = await drawn();
	await way(page, 'Review panel').click();
	await expect.poll(drawn).not.toBe(before);
	await expect(point(page, 'counts').locator('svg')).toContainText('reviewer');
	await way(page, 'Coordinator').click();
	await expect(point(page, 'work').locator('svg')).toContainText('coordinator');
	await expect(point(page, 'pick').locator('svg')).toContainText('coordinator');
});

test('the lit card is the one the rules match, and none when no card shows them', async ({
	page
}) => {
	await open(page);
	await point(page, 'counts').click();
	await page.getByLabel('It has approvals').check();
	await expect(point(page, 'counts')).toContainText("After 1 approval, not the author's.");
	await expect(way(page, 'Peer review')).toHaveAttribute('aria-pressed', 'true');
	await expect(way(page, 'Open')).toHaveAttribute('aria-pressed', 'false');
	// A review and a check together: the rule no card shows.
	await page.getByLabel('A check is reported as passed').check();
	await expect(point(page, 'counts')).toContainText(
		'After 1 approval, not the author\'s and the check "tests" reported as passed.'
	);
	await expect(page.locator('.box')).toContainText('Locust does not run the check.');
	await expect(page.locator('.ways .way[aria-pressed="true"]')).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'No problems' })).toBeVisible();
});

test('a role is made where it is needed', async ({ page }) => {
	await open(page);
	await point(page, 'pick').click();
	await page.getByLabel('One role picks one result per task').check();
	await page.getByLabel('Name of the new role').first().fill('judge');
	await page.getByRole('button', { name: 'Add role' }).click();
	await expect(point(page, 'pick')).toContainText('"judge" picks one result per task.');
	await expect(page.getByRole('button', { name: 'Role "judge"' })).toBeVisible();
	await expect(way(page, 'Independent attempts')).toHaveAttribute('aria-pressed', 'true');
	// One undo takes back the role and the rule together.
	await page.keyboard.press('Escape');
	await page.getByRole('button', { name: 'Undo' }).click();
	await expect(page.getByRole('button', { name: 'Role "judge"' })).toHaveCount(0);
	await expect(way(page, 'Open')).toHaveAttribute('aria-pressed', 'true');
});

test('copying the prompt gives a blueprint the real Locust CLI accepts', async ({ page }) => {
	test.skip(!existsSync(locust), 'no local Locust build (cargo build -p locust)');
	await open(page);
	await way(page, 'Pipeline').click();
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
	expect(Object.keys(result.result.normalized.flow)).toEqual(['draft', 'ship']);
});

test('a prompt that only checks can be copied without changing the main button', async ({
	page
}) => {
	await open(page);
	await page.getByRole('button', { name: 'Copy one that only checks' }).click();
	await expect(page.locator('.toast')).toContainText('Nothing is saved.');
	const prompt = await page.evaluate(() => navigator.clipboard.readText());
	expect(prompt).toContain('Please check this Locust blueprint with my local Locust.');
	await page.getByRole('button', { name: 'Copy prompt' }).click();
	const added = await page.evaluate(() => navigator.clipboard.readText());
	expect(added).toContain('Please add this Locust blueprint to my local Locust');
});

test('the pipeline is steps in order, each saying only what differs', async ({ page }) => {
	await open(page);
	await way(page, 'Pipeline').click();
	await expect(page.locator('.line.row')).toHaveCount(2);
	const draft = row(page, 'draft');
	const ship = row(page, 'ship');
	await expect(draft.locator('[data-point="add"]')).toContainText('Locust, at the start.');
	await expect(draft.locator('[data-point="counts"]')).toContainText(
		"After 1 approval, not the author's."
	);
	await expect(draft.locator('[data-point="work"]')).toContainText('Same as any task');
	await expect(ship.locator('[data-point="add"]')).toContainText(
		'Locust, after "draft" has a result that counts.'
	);
	await expect(ship.locator('[data-point="counts"]')).toContainText('Same as any task');
	await expect(page.locator('.roles')).toContainText('None.');
});

test('steps can be added, named, given their own answer and removed', async ({ page }) => {
	await open(page);
	await way(page, 'Pipeline').click();
	await page.getByRole('button', { name: 'Step', exact: true }).click();
	await expect(page.locator('.line.row')).toHaveCount(3);
	const added = row(page, 'step');
	await expect(added.locator('[data-point="add"]')).toContainText(
		'Locust, after "ship" has a result that counts.'
	);
	await added.getByLabel('Name of this step').fill('announce');
	await added.getByLabel('Name of this step').press('Enter');
	const announce = row(page, 'announce');
	await expect(announce).toHaveCount(1);

	// Its own answer at one point, then the same as any task again.
	await announce.locator('[data-point="counts"]').click();
	await expect(page.locator('.box')).toContainText('Same as any task.');
	await page.locator('.box').getByLabel('A check is reported as passed').check();
	await expect(announce.locator('[data-point="counts"]')).toContainText(
		'After the check "tests" reported as passed.'
	);
	await page.locator('.box').getByRole('button', { name: 'Use the same as any task' }).click();
	await expect(announce.locator('[data-point="counts"]')).toContainText('Same as any task');

	// When Locust adds it can be changed, and a loop cannot be chosen.
	await announce.locator('[data-point="add"]').click();
	await page.locator('.box').getByLabel('Which step?').selectOption('draft');
	await expect(announce.locator('[data-point="add"]')).toContainText(
		'Locust, after "draft" has a result that counts.'
	);
	await row(page, 'draft').locator('[data-point="add"]').click();
	await expect(page.locator('.box').getByLabel(/After another step/)).toBeDisabled();

	await page.getByRole('button', { name: 'Remove the step "announce"' }).click();
	await expect(page.locator('.line.row')).toHaveCount(2);
	await page.getByRole('button', { name: 'Undo' }).click();
	await expect(page.locator('.line.row')).toHaveCount(3);
});

test('another kind of task follows its own rules', async ({ page }) => {
	await open(page);
	await page.getByRole('button', { name: 'Another kind of task' }).click();
	const kind = row(page, 'kind');
	await expect(kind.locator('.cell.fixed')).toContainText('A member, as this kind of task.');
	await kind.locator('[data-point="counts"]').click();
	await page.locator('.box').getByLabel('It has approvals').check();
	await expect(kind.locator('[data-point="counts"]')).toContainText(
		"After 1 approval, not the author's."
	);
	// The rules for any task are unchanged.
	await expect(point(page, 'counts')).toContainText('When its author says so.');
	await page.getByRole('button', { name: 'In words' }).click();
	await expect(page.locator('.summary .lines')).toContainText('Tasks of the kind "kind":');
});

test('problems are explained in plain words and can be found', async ({ page }) => {
	await open(page);
	await way(page, 'Coordinator').click();
	await page.getByRole('button', { name: 'Role "coordinator"' }).click();
	await page.getByRole('button', { name: 'Remove role' }).click();
	await page.getByRole('button', { name: /^\d+ problems?$/ }).click();
	await expect(page.locator('.panel.open h2')).toHaveText(/things? to check/);
	await expect(page.locator('.problems')).toContainText('No result could ever count');
	await page.locator('.problems').getByRole('button', { name: 'Show me' }).first().click();
	await expect(page.locator('.box')).toBeVisible();
	await expect(page.locator('.line.main .problem').first()).toBeVisible();
	await page.keyboard.press('Escape');
	await page.getByRole('button', { name: 'Undo' }).click();
	await expect(page.getByRole('button', { name: 'No problems' })).toBeVisible();
});

test('a share link opens the same blueprint as a new copy', async ({ page, context }) => {
	await open(page);
	await way(page, 'Pipeline').click();
	await page.getByLabel('Name of this blueprint').fill('Sync research');
	await page.getByLabel('Name of this blueprint').press('Enter');
	await page.getByRole('button', { name: 'File' }).click();
	await page.getByRole('menuitem', { name: 'Copy a link to it' }).click();
	await expect(page.locator('.toast')).toContainText('Link copied');
	const url = await page.evaluate(() => navigator.clipboard.readText());
	expect(url).toMatch(/\/blueprints#b1\./);
	const other = await context.newPage();
	await other.goto(url);
	await expect(other.getByText('Opened from a link.')).toBeVisible();
	await expect(other.locator('.line.row')).toHaveCount(2);
	await expect(other.getByLabel('Name of this blueprint')).toHaveValue('Sync research');
	expect(other.url()).not.toContain('#');
});

test('on a phone the page fits and every answer is still in view', async ({ page }) => {
	await page.setViewportSize({ width: 375, height: 812 });
	await open(page);
	await way(page, 'Pipeline').click();
	const widths = await page.evaluate(() => [document.documentElement.scrollWidth, innerWidth]);
	expect(widths[0]).toBeLessThanOrEqual(widths[1]);
	await expect(point(page, 'work')).toContainText('No lock');
	await row(page, 'draft').locator('[data-point="counts"]').click();
	await expect(page.locator('.box')).toContainText('When does a result count?');
	await expect(row(page, 'draft').locator('.question').first()).toBeVisible();
});

test.describe('without JavaScript', () => {
	test.use({ javaScriptEnabled: false });
	test('the page still explains blueprints and links to the guide', async ({ page }) => {
		await page.goto('/blueprints');
		await expect(page.getByRole('heading', { level: 1 })).toHaveText('How does your team work?');
		await expect(page.getByRole('link', { name: 'Not set up yet? Start here.' })).toBeVisible();
	});
});
