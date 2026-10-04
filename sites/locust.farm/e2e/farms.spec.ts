import { readFileSync, mkdirSync } from 'node:fs';
import { expect, test } from '@playwright/test';
import type { FarmSnapshot } from '../src/lib/farm/types.ts';

// Shared Rust-validated synthetic fixture, confined to browser tests.
const snapshot: FarmSnapshot = JSON.parse(
	readFileSync(
		new URL('../../../crates/locust-proto/fixtures/farm-snapshot.json', import.meta.url),
		'utf8'
	)
);
const farmId = snapshot.farm_id;
const envelope = (version = 1, data = snapshot) => ({
	farm_id: farmId,
	stream_version: version,
	service_time_ms: Date.now(),
	received_at_ms: Date.now(),
	status: 'available',
	visibility: 'listed',
	snapshot: data
});

async function transport(page: import('@playwright/test').Page) {
	await page.addInitScript(() => {
		class TestSource extends EventTarget {
			static instances: TestSource[] = [];
			onopen: (() => void) | null = null;
			onerror: (() => void) | null = null;
			constructor() {
				super();
				TestSource.instances.push(this);
				setTimeout(() => this.onopen?.(), 0);
			}
			close() {
				TestSource.instances = TestSource.instances.filter((item) => item !== this);
			}
		}
		Object.assign(window, { EventSource: TestSource, farmStreams: TestSource });
	});
	await page.route('**/api/farms*', (route) =>
		route.fulfill({ json: { farms: [envelope()], next_cursor: null } })
	);
	await page.route('**/api/farms/*', (route) => route.fulfill({ json: envelope() }));
}
async function push(page: import('@playwright/test').Page, data: object) {
	await page.evaluate((data) => {
		const streams = (window as unknown as { farmStreams: { instances: EventTarget[] } }).farmStreams
			.instances;
		for (const stream of streams)
			stream.dispatchEvent(new MessageEvent('state', { data: JSON.stringify(data) }));
	}, data);
}

for (const width of [1440, 390])
	test(`farm preserves stages, work facts and keyboard access at ${width}px`, async ({ page }) => {
		await page.setViewportSize({ width, height: 1000 });
		await page.emulateMedia({ reducedMotion: 'reduce' });
		const errors: string[] = [];
		page.on('pageerror', (error) => errors.push(error.message));
		await transport(page);
		await page.goto(`/farm/${farmId}`);
		await expect(page.getByRole('heading', { level: 1 })).toHaveText(snapshot.title!);
		await expect(page.locator('.edges .edge')).toHaveCount(5);
		await expect(page.locator('.map .cols > .col')).toHaveCount(4); // 4 topology levels; unstaged work is separate.
		await expect(page.locator('.step[data-stage="0"]')).toContainText('Unstaged work');
		await expect(page.locator('.step[data-stage="2"] .agent').first()).toContainText('0003, 0004');
		await expect(page.locator('.agents-table').last()).toContainText('unknown');
		const mark = page.getByRole('button', { name: 'Task 0004, Awaiting evidence' });
		await mark.focus();
		await page.keyboard.press('Enter');
		await expect(page.locator('[data-task="0004"]')).toHaveClass(/selected-row/);
		mkdirSync('../../output/farm-ui', { recursive: true });
		await page.screenshot({ path: `../../output/farm-ui/farm-${width}.png`, fullPage: true });
		await page.getByRole('button', { name: 'Pause updates' }).click();
		await push(page, { ...envelope(2), status: 'unavailable', snapshot: null });
		await expect(page.getByRole('heading', { level: 1 })).toHaveText('Farm unavailable');
		await expect(page.locator('.map')).toHaveCount(0);
		await expect(page.locator('body')).not.toContainText('Contribution published');
		expect(errors).toEqual([]);
	});

test('gallery shares the stage layout, filters without rearranging cards and clears unavailable farms', async ({
	page
}) => {
	await transport(page);
	await page.goto('/farms');
	await expect(page.locator('.farm-card')).toHaveCount(1);
	await expect(page.locator('.farm-card .edges .edge')).toHaveCount(5);
	await page.screenshot({ path: '../../output/farm-ui/gallery-1440.png', fullPage: true });
	await page.setViewportSize({ width: 390, height: 1000 });
	await page.screenshot({ path: '../../output/farm-ui/gallery-390.png', fullPage: true });
	await page.getByRole('button', { name: 'Quiet', exact: true }).click();
	await expect(page.locator('.farm-card')).toHaveCount(0);
	await page.getByRole('button', { name: 'All', exact: true }).click();
	await expect(page.locator('.farm-card')).toHaveCount(1);
	await push(page, { ...envelope(2), status: 'unavailable', snapshot: null });
	await expect(page.locator('.farm-card')).toHaveCount(0);
});

test('quiet, ended, disconnected and hostile text states are literal', async ({ page }) => {
	await transport(page);
	await page.goto(`/farm/${farmId}`);
	await expect(page.getByRole('heading', { level: 1 })).toHaveText(snapshot.title!);
	await push(page, { ...envelope(2), received_at_ms: Date.now() - 180000 });
	await expect(page.locator('.status .state')).toHaveText('Quiet');
	await push(
		page,
		envelope(3, {
			...snapshot,
			goal_state: 'ended',
			title: '<script>window.injected=true</script>'
		})
	);
	await expect(page.locator('.status .state')).toHaveText('Ended');
	await expect(page.getByRole('heading', { level: 1 })).toHaveText(
		'<script>window.injected=true</script>'
	);
	expect(await page.evaluate(() => 'injected' in window)).toBe(false);
	await page.evaluate(() => {
		for (const stream of (
			window as unknown as { farmStreams: { instances: { onerror: () => void }[] } }
		).farmStreams.instances)
			stream.onerror();
	});
	await expect(
		page.getByRole('status').filter({ hasText: 'Connection interrupted' })
	).toBeVisible();
});

test('an unlisted farm disappears and a stale listing response cannot reinsert it', async ({
	page
}) => {
	await transport(page);
	await page.goto('/farms');
	await expect(page.locator('.farm-card')).toHaveCount(1);
	await push(page, { ...envelope(2), visibility: 'link' });
	await expect(page.locator('.farm-card')).toHaveCount(0);
	await page.getByRole('button', { name: 'Quiet', exact: true }).click();
	await page.getByRole('button', { name: 'All', exact: true }).click();
	await expect(page.getByText('No farms are listed yet.', { exact: false })).toBeVisible();
	await expect(page.locator('.farm-card')).toHaveCount(0);
});

test('closed completed tasks retain their completion count independently of closure', async ({
	page
}) => {
	await transport(page);
	await page.goto(`/farm/${farmId}`);
	await expect(page.getByRole('heading', { level: 1 })).toHaveText(snapshot.title!);
	const closed = structuredClone(snapshot);
	closed.tasks[0].closed = true;
	closed.tasks[0].state = 'closed';
	await push(page, envelope(2, closed));
	await expect(page.locator('.count.done .n')).toContainText('2');
	await expect(page.locator('.count-note')).toContainText('1 closed');
});
