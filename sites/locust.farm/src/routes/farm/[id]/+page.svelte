<script lang="ts">
	import { page } from '$app/state';
	import { onMount } from 'svelte';
	import SiteHeader from '#lib/components/SiteHeader.svelte';
	import FarmMap from '#lib/farm/FarmMap.svelte';
	import { watchFarm, serviceNow, type FarmState, type Connection } from '#lib/farm/client.ts';
	import {
		changeText,
		farmMode,
		harnessName,
		mapState,
		modeName,
		taskFacts
	} from '#lib/farm/model.ts';
	import { age, clockTime } from '#lib/farm/time.ts';
	import '#lib/farm/farm.css';
	let view = $state<FarmState | null>(null);
	let connection = $state<Connection>('connecting');
	let now = $state(Date.now());
	let paused = $state(false);
	let pending = $state<FarmState | null>(null);
	let selected = $state<number | null>(null);
	let copied = $state('');
	let allChanges = $state(false);
	const snapshot = $derived(view?.snapshot);
	const mode = $derived(view ? farmMode(view, now) : 'quiet');
	const counts = $derived(
		snapshot
			? {
					open: snapshot.tasks.filter((task) => task.state === 'open').length,
					reported: snapshot.tasks.filter((task) => task.state === 'reported').length,
					waiting: snapshot.tasks.filter((task) => task.state === 'awaiting_evidence').length,
					done: snapshot.tasks.filter((task) => task.completed).length,
					closed: snapshot.tasks.filter((task) => task.state === 'closed').length
				}
			: null
	);
	$effect(() => {
		const id = page.params.id;
		if (!id) return;
		view = null;
		pending = null;
		selected = null;
		return watchFarm(
			id,
			(next) => {
				// Revocation is immediate even when a visitor paused visual updates.
				if (!next.snapshot || next.status === 'unavailable') {
					view = next;
					pending = null;
				} else if (paused) pending = next;
				else view = next;
			},
			(next) => {
				connection = next;
			}
		);
	});
	onMount(() => {
		const timer = setInterval(() => {
			now = Date.now();
		}, 1000);
		return () => clearInterval(timer);
	});
	function togglePause() {
		paused = !paused;
		if (!paused && pending) {
			view = pending;
			pending = null;
		}
	}
	async function copyLink() {
		try {
			await navigator.clipboard.writeText(page.url.href);
			copied = 'Link copied';
		} catch {
			copied = 'Copy the address from your browser';
		}
	}
</script>

<svelte:head>
	<title>{snapshot?.title ?? 'Farm'} · locust.farm</title>
	<meta name="robots" content="noindex" />
	<meta name="referrer" content="no-referrer" />
</svelte:head>
<div class="farm-site ui" data-mode={mode}>
	<SiteHeader />
	<main>
		{#if !snapshot}
			<section class="empty" aria-live="polite">
				<h1>
					{view
						? 'Farm unavailable'
						: connection === 'disconnected'
							? 'Can’t reach this farm'
							: 'Loading farm'}
				</h1>
				<p>
					{view
						? 'This farm is offline.'
						: connection === 'disconnected'
							? 'Connection lost. Reconnecting…'
							: 'Waiting for the first update.'}
				</p>
				<a class="button" href="/farms">Browse farms</a>
			</section>
		{:else}
			<div class="head">
				<div class="intro">
					<p class="eyebrow">Farm · goal {snapshot.goal_state}</p>
					<h1 class="title">{snapshot.title ?? 'Untitled farm'}</h1>
					<p class="meta">
						<span class="line">{snapshot.formation} · {snapshot.stages.length} stages</span><span
							class="line"
							>{snapshot.agents.length} agents · {snapshot.groups.length}
							{snapshot.groups.length === 1 ? 'group' : 'groups'}</span
						>
					</p>
				</div>
				<section class="status card" aria-label="Publisher status">
					<p class="state">
						<span class="status-mark"></span>{snapshot.goal_state === 'disputed'
							? 'Goal disputed'
							: snapshot.goal_state === 'unavailable'
								? 'Goal state unavailable'
								: modeName(mode)}
					</p>
					{#if mode !== 'ended'}
						<p class="status-text">
							Last check-in <b>{age(view?.received_at_ms, serviceNow(view, now))}</b>
						</p>
					{/if}
					<p class="status-text">
						Updated <b>{age(snapshot.observed_at_ms, serviceNow(view, now))}</b>
					</p>
					<div class="actions">
						<button onclick={togglePause}>{paused ? 'Resume updates' : 'Pause updates'}</button
						><button onclick={copyLink}>Copy link</button><span class="copy-result" role="status"
							>{copied}</span
						>
					</div>
				</section>
			</div>
			{#if connection !== 'connected' || paused}<p class="connection-note" role="status">
					{paused ? 'Updates paused.' : 'Connection interrupted. Reconnecting…'}
				</p>{/if}
			<div class="counts" aria-label="Task summary">
				{#each [['Open', counts!.open], ['Attempted', counts!.reported], ['Awaiting evidence', counts!.waiting]] as [label, count] (label)}<div
						class="count"
					>
						<span class="label">{label}</span><span class="n">{count}</span>
					</div>{/each}
				<div class="count done">
					<span class="label">Completed</span><span class="n"
						><strong>{counts!.done}</strong> <small>of {snapshot.tasks.length}</small></span
					>
				</div>
				<p class="count-note">
					{counts!.closed} closed · {snapshot.tasks.filter(
						(task) => task.state === 'unavailable' || task.state === 'disputed'
					).length} unavailable or disputed
				</p>
			</div>
			<section class="map-wrap" aria-label="Formation map">
				<FarmMap
					{snapshot}
					{selected}
					onSelect={(id) => {
						selected = selected === id ? null : id;
					}}
				/>
				<div class="key">
					<span><i class="dot"></i> open</span><span><i class="dot taken"></i> attempted</span><span
						><i class="dot waiting"></i> awaiting evidence</span
					><span><i class="dot done"></i> completed</span><span>□ closed</span><span
						>◇ disputed</span
					><span>◌ unavailable</span>
				</div>
				<p class="rule">Select a task to see its details below.</p>
			</section>
			<div class="lower">
				<section>
					<div class="section-head">
						<h2>Tasks</h2>
						<p>
							{snapshot.tasks.length}
							{snapshot.tasks.length === 1 ? 'task' : 'tasks'}
						</p>
					</div>
					<table class="work-table stack">
						<thead
							><tr
								><th aria-label="State mark"></th><th>Task</th><th>Status</th><th>Attempts</th></tr
							></thead
						><tbody>
							{#each [...snapshot.stages, { id: 0, label: 'Unstaged work', prerequisites: [] }] as stage (stage.id)}
								{@const tasks = snapshot.tasks.filter((task) => (task.stage ?? 0) === stage.id)}
								{#if tasks.length || stage.id !== 0}<tr class="group"
										><td colspan="4">{stage.id ? `Stage ${stage.id} · ` : ''}{stage.label}</td></tr
									>{/if}
								{#each tasks as task (task.id)}
									{@const attempts = snapshot.attempts.filter(
										(attempt) => attempt.task === task.id && attempt.round === task.round
									)}
									<tr class:selected-row={selected === task.id} data-task={task.reference}>
										<td class="cell-mark"
											><span class={`dot ${mapState(task)}`} aria-hidden="true"></span></td
										>
										<td class="strong"
											><button
												class="ref-btn"
												onclick={() => {
													selected = selected === task.id ? null : task.id;
												}}>{task.reference}</button
											></td
										>
										<td
											><div class="facts">
												{#each taskFacts(task) as fact (fact)}<span>{fact}</span
													>{/each}{#each snapshot.candidates.filter((candidate) => candidate.task === task.id && candidate.round === task.round) as candidate (candidate.id)}<small
														>Candidate {candidate.id}: {candidate.completed
															? 'complete'
															: 'not complete'} ·
														{candidate.evidence_count}
														{candidate.evidence_count === 1
															? 'evidence record'
															: 'evidence records'} ·
														{candidate.requirement}</small
													>{/each}
											</div></td
										>
										<td data-label="Attempts:"
											><div class="facts">
												{#each attempts as attempt (attempt.id)}{@const agent =
														snapshot.agents.find((agent) => agent.id === attempt.agent)}<span
														>{agent?.name ?? `Agent ${attempt.agent}`} · {attempt.state}</span
													>{:else}<span>None</span>{/each}
											</div></td
										>
									</tr>
								{/each}
							{/each}
						</tbody>
					</table>
					{#if !snapshot.tasks.length}<p class="note">No tasks yet.</p>{/if}
				</section>
				<section class="heard">
					<div class="section-head">
						<h2>Recent changes</h2>
					</div>
					<ol>
						{#each [...snapshot.changes]
							.sort((a, b) => b.id - a.id)
							.slice(0, allChanges ? undefined : 8) as change (change.id)}<li>
								<time>{clockTime(change.observed_at_ms)}</time><span>{changeText(change)}</span>
							</li>{:else}<li>No changes yet.</li>{/each}
					</ol>
					{#if snapshot.changes.length > 8}<button
							class="button quiet"
							onclick={() => {
								allChanges = !allChanges;
							}}
							>{allChanges
								? 'Show fewer changes'
								: `Show all ${snapshot.changes.length} changes`}</button
						>{/if}
					{#if snapshot.omitted_changes}<p class="note">
							{snapshot.omitted_changes} older changes are not shown.
						</p>{/if}
				</section>
				<section>
					<div class="section-head">
						<h2>Agents</h2>
					</div>
					<table class="agents-table stack">
						<thead><tr><th>Agent</th><th>Roles</th><th>Attempts</th></tr></thead><tbody
							>{#each snapshot.agents as agent (agent.id)}<tr
									><td class="strong"
										>{agent.id} · {agent.name} · {harnessName(agent.harness)}<span class="sub"
											>Group {agent.group}</span
										></td
									><td data-label="Roles:">{agent.roles.join(', ') || 'None'}</td><td
										data-label="Attempts:"
										>{snapshot.attempts
											.filter((attempt) => attempt.agent === agent.id)
											.map(
												(attempt) =>
													`${snapshot.tasks.find((task) => task.id === attempt.task)?.reference ?? '?'} r${attempt.round} · ${attempt.state}`
											)
											.join('; ') || 'None'}</td
									></tr
								>{/each}</tbody
						>
					</table>
				</section>
				<section>
					<div class="section-head">
						<h2>Groups</h2>
					</div>
					<table class="stack agents-table">
						<thead><tr><th>Group</th><th>Agents</th><th>Last sync</th></tr></thead><tbody
							>{#each snapshot.groups as group (group.id)}<tr
									><td class="strong">{group.id} · {group.label ?? 'Unnamed group'}</td><td
										data-label="Agents:"
										>{snapshot.agents.filter((agent) => agent.group === group.id).length}</td
									><td data-label="Last sync:"
										>{age(group.last_sync_at_ms, serviceNow(view, now))}</td
									></tr
								>{/each}</tbody
						>
					</table>
				</section>
			</div>
		{/if}
	</main>
</div>
