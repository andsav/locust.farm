<script lang="ts">
	import { onMount } from 'svelte';
	import { SvelteMap, SvelteSet, SvelteURLSearchParams } from 'svelte/reactivity';
	import FarmHeader from '#lib/farm/FarmHeader.svelte';
	import FarmMap from '#lib/farm/FarmMap.svelte';
	import {
		watchFarm,
		serviceNow,
		type FarmState,
		type FarmListing,
		type Connection
	} from '#lib/farm/client.ts';
	import { farmMode, modeName } from '#lib/farm/model.ts';
	import { age } from '#lib/farm/time.ts';
	import '#lib/farm/farm.css';
	let farms = $state<FarmState[]>([]);
	let filter = $state('all');
	let cursor = $state<string | null>(null);
	let loading = $state(true);
	let error = $state(false);
	let now = $state(Date.now());
	let active = false;
	let generation = 0;
	const subscriptions = new SvelteMap<string, () => void>();
	const connections = new SvelteMap<string, Connection>();
	const knownVersions = new SvelteMap<string, number>();
	const hidden = new SvelteSet<string>();
	const visible = $derived(
		farms.filter((state) => state.snapshot && (filter === 'all' || farmMode(state, now) === filter))
	);
	async function load(more = false) {
		const token = ++generation;
		loading = true;
		error = false;
		try {
			const query = new SvelteURLSearchParams({ page_size: '24', filter });
			if (more && cursor) query.set('cursor', cursor);
			const response = await fetch(`/api/farms?${query}`, { cache: 'no-store' });
			if (!response.ok) throw new Error('Unavailable');
			const listing: FarmListing = await response.json();
			if (!active || token !== generation) return;
			cursor = listing.next_cursor;
			for (const farm of listing.farms) {
				if (!farm.snapshot) continue;
				farm.local_received_at_ms = Date.now();
				const known = knownVersions.get(farm.farm_id);
				if (
					known != null &&
					(farm.stream_version < known ||
						(farm.stream_version === known && hidden.has(farm.farm_id)))
				)
					continue;
				knownVersions.set(farm.farm_id, farm.stream_version);
				hidden.delete(farm.farm_id);
				const index = farms.findIndex((state) => state.farm_id === farm.farm_id);
				if (index < 0) farms.push(farm);
				else if (farm.stream_version > farms[index].stream_version) farms[index] = farm;
				if (!subscriptions.has(farm.farm_id))
					subscriptions.set(
						farm.farm_id,
						watchFarm(
							farm.farm_id,
							(next) => {
								const position = farms.findIndex((state) => state.farm_id === next.farm_id);
								if (position < 0 || next.stream_version < farms[position].stream_version) return;
								knownVersions.set(next.farm_id, next.stream_version);
								if (!next.snapshot || next.status === 'unavailable' || next.visibility === 'link') {
									hidden.add(next.farm_id);
									farms.splice(position, 1);
									subscriptions.get(next.farm_id)?.();
									subscriptions.delete(next.farm_id);
								} else farms[position] = next;
							},
							(next) => {
								connections.set(farm.farm_id, next);
							}
						)
					);
			}
		} catch {
			if (active && token === generation) error = true;
		} finally {
			if (active && token === generation) loading = false;
		}
	}
	onMount(() => {
		active = true;
		void load();
		const timer = setInterval(() => {
			now = Date.now();
		}, 1000);
		return () => {
			active = false;
			generation++;
			clearInterval(timer);
			for (const stop of subscriptions.values()) stop();
		};
	});
</script>

<svelte:head
	><title>Farms · locust.farm</title><meta
		name="description"
		content="Public farm pages whose creators chose to list them."
	/><meta name="referrer" content="no-referrer" /></svelte:head
>
<div class="farm-site">
	<FarmHeader gallery />
	<main>
		<div class="gallery-intro">
			<h1 class="display gallery-title">Farms</h1>
			<p>
				Swarms whose creators chose to list their farm page here. Each picture shows the last view
				the creator’s Locust sent, and changes only when an update arrives.
			</p>
			<p>
				A farm shared only by its link is never listed here. Everyone covered by a published view
				agreed to its disclosure policy.
			</p>
		</div>
		<div class="gallery-controls">
			<div class="filters" aria-label="Filter farms">
				{#each [['all', 'All'], ['receiving', 'Receiving updates'], ['quiet', 'Quiet'], ['ended', 'Ended']] as [value, label] (value)}<button
						aria-pressed={filter === value}
						onclick={() => {
							filter = value;
							void load();
						}}>{label}</button
					>{/each}
			</div>
			<p class="rule">Latest change first. Farms keep their places while you read.</p>
		</div>
		{#if error}<p class="connection-note" role="status">
				Unable to reach the farms service. <button class="btn" onclick={() => load()}
					>Try again</button
				>
			</p>{/if}
		<div class="gallery-grid">
			{#each visible as farm (farm.farm_id)}
				{@const snapshot = farm.snapshot!}
				{@const mode = farmMode(farm, now)}
				<a
					class="farm-card"
					href={`/farm/${farm.farm_id}`}
					data-mode={mode}
					aria-label={`${snapshot.title ?? 'Title not shared'} · ${modeName(mode)}`}
				>
					<FarmMap {snapshot} compact />
					<div>
						<h2>{snapshot.title ?? 'Title not shared'}</h2>
						<p>{snapshot.agents.length} agents · {snapshot.groups.length} Locust groups</p>
						<p>
							{snapshot.formation} · {snapshot.stages.length
								? `${snapshot.stages.length} stages`
								: 'unstaged work'}
						</p>
						<p>
							<strong>{snapshot.tasks.filter((task) => task.completed).length}</strong>
							of {snapshot.tasks.length} completed · {snapshot.tasks.filter(
								(task) => task.state === 'reported'
							).length} with reported attempts
						</p>
					</div>
					<div>
						<p class="state"><span class="status-mark"></span>{modeName(mode)}</p>
						<p>Last service receipt {age(farm.received_at_ms, serviceNow(farm, now))}</p>
						{#if connections.get(farm.farm_id) === 'disconnected'}<p>
								Connection interrupted · checking publication again on reconnect
							</p>{/if}
					</div>
				</a>
			{/each}
		</div>
		{#if loading}<p role="status">Loading farms…</p>{:else if !visible.length && !error}<p
				class="connection-note"
			>
				{farms.length
					? 'No farms match this filter.'
					: 'No farms are listed yet. A creator can publish a farm privately by link or choose to list it here.'}
			</p>{/if}
		{#if cursor}<div>
				<button class="btn" disabled={loading} onclick={() => load(true)}>Load more farms</button>
			</div>{/if}
		<div class="about">
			<section>
				<h2>What the pictures show</h2>
				<p>
					Each stage has one mark per task and numbered agent associations. Parallel branches stay
					parallel. A ring is open, grey has reported attempts, white awaits evidence and ember is
					completed.
				</p>
			</section>
			<section>
				<h2>What the states mean</h2>
				<p>
					Receiving updates means a service receipt within two minutes. Quiet means no recent
					receipt; work may still be happening. Ended requires an explicit goal closure.
				</p>
			</section>
			<section>
				<h2>What stays private</h2>
				<p>
					Task text, results, code, prompts, tool activity, costs, keys, addresses and file paths.
					Lost publication eligibility makes the farm unavailable when suspension reaches the
					service.
				</p>
			</section>
		</div>
	</main>
</div>
