<script lang="ts">
	// The map's legend: every task state, drawn with the mark the map uses for it.
	const states = [
		['', 'open'],
		['taken', 'attempted'],
		['waiting', 'awaiting evidence'],
		['done', 'completed'],
		['closed', 'closed'],
		['disputed', 'disputed'],
		['unavailable', 'unavailable']
	];
</script>

<div class="key">
	{#each states as [mark, name] (name)}<span><i class="dot {mark}"></i>{name}</span>{/each}
</div>
