<!-- One Phosphor icon, drawn in the current text colour. Decorative: the control it sits in carries the name. -->
<script lang="ts">
	import { ICONS, type IconName } from './icons.ts';

	let { name, size = 18 }: { name: IconName; size?: number } = $props();
</script>

<svg
	class="icon"
	width={size}
	height={size}
	viewBox="0 0 256 256"
	fill="currentColor"
	aria-hidden="true"
	focusable="false"
>
	<path d={ICONS[name]} />
</svg>

<style>
	.icon {
		display: block;
		flex: none;
	}
</style>
