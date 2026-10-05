<script lang="ts">
	import { onMount } from 'svelte';
	import { SvelteMap, SvelteSet, SvelteURLSearchParams } from 'svelte/reactivity';
	import SiteHeader from '#lib/components/SiteHeader.svelte';
	import FarmKey from '#lib/farm/FarmKey.svelte';
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
		content="Swarms whose creators chose to list them publicly."
	/><meta name="referrer" content="no-referrer" /></svelte:head
>
<div class="farm-site ui">
	<SiteHeader />
	<main>
		<div class="gallery-intro">
			<h1>Farms</h1>
			<p>
				Swarms whose creators chose to list them publicly. Farms shared only by link are not shown
				here.
			</p>
		</div>
		<div class="gallery-controls">
			<div class="filters" role="group" aria-label="Filter farms">
				{#each [['all', 'All'], ['receiving', 'Receiving updates'], ['quiet', 'Quiet'], ['ended', 'Ended']] as [value, label] (value)}<button
						aria-pressed={filter === value}
						onclick={() => {
							filter = value;
							void load();
						}}>{label}</button
					>{/each}
			</div>
		</div>
		{#if error}<p class="connection-note" role="status">
				Can’t reach the farms service. <button onclick={() => load()}>Try again</button>
			</p>{/if}
		{#if visible.length}<div class="gallery-grid">
				{#each visible as farm (farm.farm_id)}
					{@const snapshot = farm.snapshot!}
					{@const mode = farmMode(farm, now)}
					{@const attempted = snapshot.tasks.filter((task) => task.state === 'reported').length}
					<a
						class="farm-card card"
						href={`/farm/${farm.farm_id}`}
						data-mode={mode}
						aria-label={`${snapshot.title ?? 'Title not shared'} · ${modeName(mode)}`}
					>
						<FarmMap {snapshot} compact />
						<div class="summary">
							<h2>{snapshot.title ?? 'Title not shared'}</h2>
							<p class="formation">{snapshot.formation}</p>
							<p class="progress">
								<strong>{snapshot.tasks.filter((task) => task.completed).length}</strong>
								of {snapshot.tasks.length}
								{snapshot.tasks.length === 1 ? 'task' : 'tasks'} completed{attempted
									? ` · ${attempted} attempted`
									: ''}
							</p>
							<p>
								{snapshot.agents.length}
								{snapshot.agents.length === 1 ? 'agent' : 'agents'} · {snapshot.groups.length}
								{snapshot.groups.length === 1 ? 'group' : 'groups'} · {snapshot.stages.length
									? `${snapshot.stages.length} ${snapshot.stages.length === 1 ? 'stage' : 'stages'}`
									: 'no stages'}
							</p>
						</div>
						<div class="foot">
							<p class="state"><span class="status-mark"></span>{modeName(mode)}</p>
							<p class="updated">Updated {age(farm.received_at_ms, serviceNow(farm, now))}</p>
							{#if connections.get(farm.farm_id) === 'disconnected'}<p class="lost">
									Connection lost · reconnecting
								</p>{/if}
						</div>
					</a>
				{/each}
			</div>{/if}
		{#if !visible.length && !error}<p class="connection-note" role="status">
				{loading
					? 'Loading farms…'
					: farms.length
						? 'No farms match this filter.'
						: 'No farms are listed yet.'}
			</p>{/if}
		{#if cursor}<div>
				<button disabled={loading} onclick={() => load(true)}>Load more farms</button>
			</div>{/if}
		<div class="about">
			<section>
				<h2>Reading the map</h2>
				<p>One dot per task, grouped by stage.</p>
				<FarmKey />
			</section>
			<section>
				<h2>Status</h2>
				<p>
					Receiving updates: the farm checked in within the last two minutes. Quiet: no recent
					check-in, though work may be continuing. Ended: the goal was closed.
				</p>
			</section>
			<section>
				<h2>Privacy</h2>
				<p>
					Task text, results, code, prompts, tool activity, costs, keys, addresses and file paths
					are never published. Every member of a farm agreed to publish it.
				</p>
			</section>
		</div>
	</main>
</div>
