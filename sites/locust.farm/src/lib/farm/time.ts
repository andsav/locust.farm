/** Observation timestamps are optional: imported history must stay unknown. */
export function age(timestamp: number | null | undefined, now: number): string {
	if (
		timestamp == null ||
		!Number.isFinite(timestamp) ||
		!Number.isFinite(now) ||
		Math.abs(timestamp) > 8.64e15
	)
		return 'unknown';
	if (timestamp > now) return 'ahead of service clock';
	const seconds = Math.floor((now - timestamp) / 1000);
	if (seconds < 60) return `${seconds} s ago`;
	const minutes = Math.floor(seconds / 60);
	if (minutes < 60) return `${minutes} min ago`;
	const hours = Math.floor(minutes / 60);
	if (hours < 24) return `${hours} h ago`;
	return `${Math.floor(hours / 24)} d ago`;
}

export function clockTime(timestamp: number | null | undefined): string {
	return timestamp == null || !Number.isFinite(new Date(timestamp).getTime())
		? 'unknown'
		: new Date(timestamp).toLocaleTimeString([], {
				hour: '2-digit',
				minute: '2-digit',
				hourCycle: 'h23'
			});
}
