<!--
	One step of "How does it work?" as a small picture. A box is a computer and the orange
	square in it is that computer's copy of the goal. Grey dots are work on the move, white
	dots are results and orange marks a decision. The pictures are plain SVG and CSS, so
	they are prerendered. With reduced motion each one holds a frame that still reads.
-->
<script lang="ts" module>
	type Point = readonly [x: number, y: number];
	type Tone = 'work' | 'result' | 'decision';

	/** Seconds for one loop of each picture. */
	const CYCLE = { agent: 6, goal: 7, sync: 8, review: 8 };

	export type Step = keyof typeof CYCLE;

	const AGENT: Point = [70, 76];
	const LOCUST: Point = [134, 76];
	/** Past the right edge, where the picture clips. */
	const OUTSIDE: Point = [248, 76];

	const YOU: Point = [52, 68];
	const INVITED: Point[] = [
		[188, 38],
		[188, 112]
	];

	const TOP: Point = [120, 36];
	const LEFT: Point = [50, 112];
	const RIGHT: Point = [190, 112];

	const AUTHOR: Point = [58, 66];
	const REVIEWER: Point = [182, 66];
</script>

<script lang="ts">
	let { step }: { step: Step } = $props();
</script>

<svg viewBox="0 0 240 150" aria-hidden="true" style:--cycle="{CYCLE[step]}s">
	{#snippet link(a: Point, b: Point, open = false)}
		<path class="link" class:open d="M{a[0]} {a[1]}L{b[0]} {b[1]}" />
	{/snippet}

	<!-- `at` is the moment of the loop, from 0 to 1, when the dot leaves. -->
	{#snippet dot(from: Point, to: Point, at: number, tone: Tone, rest = false)}
		<circle
			class="dot {tone}"
			class:rest
			r="2.6"
			style:--x0="{from[0]}px"
			style:--y0="{from[1]}px"
			style:--x1="{to[0]}px"
			style:--y1="{to[1]}px"
			style:--at={at}
		/>
	{/snippet}

	<!-- With `arrives`, the copy of the goal appears at that moment of the loop. -->
	{#snippet computer(at: Point, label = '', arrives: number | null = null)}
		<rect class="computer" x={at[0] - 24} y={at[1] - 18} width="48" height="36" rx="6" />
		<rect
			class="goal"
			class:arrives={arrives !== null}
			x={at[0] - 4}
			y={at[1] - 4}
			width="8"
			height="8"
			rx="1.5"
			style:--at={arrives}
		/>
		{#if label}<text x={at[0]} y={at[1] + 33}>{label}</text>{/if}
	{/snippet}

	{#if step === 'agent'}
		<rect class="computer" x="26" y="42" width="152" height="82" rx="8" />
		<text class="start" x="27" y="33">your computer</text>
		{@render link(AGENT, LOCUST)}
		{@render link(LOCUST, OUTSIDE, true)}
		{@render dot(AGENT, LOCUST, 0.02, 'work', true)}
		{@render dot(LOCUST, OUTSIDE, 0.18, 'work', true)}
		{@render dot(OUTSIDE, LOCUST, 0.5, 'result')}
		{@render dot(LOCUST, AGENT, 0.66, 'result')}
		<circle class="node" cx={AGENT[0]} cy={AGENT[1]} r="9" />
		<circle class="node lead" cx={LOCUST[0]} cy={LOCUST[1]} r="9" />
		<text x={AGENT[0]} y={AGENT[1] + 27}>agent</text>
		<text x={LOCUST[0]} y={LOCUST[1] + 27}>locust</text>
		<text x="209" y="65">peers</text>
	{:else if step === 'goal'}
		{#each INVITED as member, i (i)}
			{@render link(YOU, member)}
			{@render dot(YOU, member, 0.06 + i * 0.08, 'decision')}
		{/each}
		{@render computer(YOU, 'you')}
		{#each INVITED as member, i (i)}
			{@render computer(member, '', 0.2 + i * 0.08)}
		{/each}
	{:else if step === 'sync'}
		{@render link(TOP, LEFT)}
		{@render link(LEFT, RIGHT)}
		{@render link(RIGHT, TOP)}
		{@render dot(TOP, LEFT, 0, 'work', true)}
		{@render dot(LEFT, RIGHT, 0.12, 'result')}
		{@render dot(RIGHT, TOP, 0.24, 'work', true)}
		{@render dot(LEFT, TOP, 0.38, 'result')}
		{@render dot(TOP, RIGHT, 0.5, 'work')}
		{@render dot(RIGHT, LEFT, 0.62, 'result', true)}
		{@render dot(LEFT, RIGHT, 0.76, 'work')}
		{@render dot(RIGHT, TOP, 0.88, 'result')}
		{@render computer(TOP)}
		{@render computer(LEFT)}
		{@render computer(RIGHT)}
	{:else}
		{@render link(AUTHOR, REVIEWER)}
		{@render dot(AUTHOR, REVIEWER, 0.04, 'result')}
		{@render dot(REVIEWER, AUTHOR, 0.42, 'decision', true)}
		{@render computer(AUTHOR, 'author')}
		{@render computer(REVIEWER, 'reviewer')}
		<rect
			class="reviewing"
			x={REVIEWER[0] - 29}
			y={REVIEWER[1] - 23}
			width="58"
			height="46"
			rx="10"
			style:--at={0.18}
		/>
		<g transform="translate({AUTHOR[0]} {AUTHOR[1]})">
			<circle class="accepted" r="9" style:--at={0.56} />
		</g>
	{/if}
</svg>

<style>
	svg {
		display: block;
		width: 100%;
		height: auto;
	}

	.computer {
		fill: var(--color-surface);
		stroke: var(--color-text-faint);
	}

	.node {
		fill: var(--color-bg);
		stroke: var(--color-text-faint);
	}

	.lead {
		stroke: var(--color-accent);
	}

	.link {
		fill: none;
		stroke: var(--color-text-faint);
	}

	/* A connection that leaves the picture. */
	.open {
		stroke-dasharray: 2 4;
	}

	.goal {
		fill: var(--color-accent);
	}

	text {
		fill: var(--color-text-subtle);
		font: var(--text-label);
		font-size: 9px;
		letter-spacing: var(--tracking-label);
		text-anchor: middle;
		text-transform: uppercase;
	}

	.start {
		text-anchor: start;
	}

	.work {
		fill: var(--color-text-subtle);
	}

	.result {
		fill: var(--color-text);
	}

	.decision {
		fill: var(--color-accent);
	}

	.reviewing,
	.accepted {
		fill: none;
		stroke-width: 1.2;
		opacity: 0;
	}

	.reviewing {
		stroke: var(--color-text-subtle);
	}

	.accepted {
		stroke: var(--color-accent);
		vector-effect: non-scaling-stroke;
	}

	/* Every animation lasts one loop and starts part-way in, so the picture is already
	   under way when the page opens. Without motion, a dot rests half-way along its path. */
	.dot,
	.arrives,
	.reviewing,
	.accepted {
		animation-duration: var(--cycle);
		animation-delay: calc((var(--at) - 1) * var(--cycle));
		animation-iteration-count: infinite;
	}

	.dot {
		opacity: 0;
		transform: translate(calc((var(--x0) + var(--x1)) / 2), calc((var(--y0) + var(--y1)) / 2));
		animation-name: travel;
	}

	.arrives {
		animation-name: arrive;
	}

	.reviewing {
		animation-name: review;
	}

	.accepted {
		animation-name: accept;
	}

	@keyframes travel {
		0% {
			transform: translate(var(--x0), var(--y0));
			opacity: 1;
			animation-timing-function: ease-in-out;
		}
		14% {
			transform: translate(var(--x1), var(--y1));
			opacity: 1;
		}
		14.01%,
		100% {
			transform: translate(var(--x1), var(--y1));
			opacity: 0;
		}
	}

	@keyframes arrive {
		0% {
			opacity: 0;
		}
		3%,
		62% {
			opacity: 1;
		}
		66%,
		100% {
			opacity: 0;
		}
	}

	@keyframes review {
		0% {
			opacity: 0;
		}
		3%,
		21% {
			opacity: 1;
		}
		24%,
		100% {
			opacity: 0;
		}
	}

	@keyframes accept {
		0% {
			transform: scale(1);
			opacity: 0.9;
		}
		14%,
		100% {
			transform: scale(2.4);
			opacity: 0;
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.dot,
		.arrives,
		.reviewing,
		.accepted {
			animation: none;
		}

		.rest {
			opacity: 1;
		}
	}
</style>
