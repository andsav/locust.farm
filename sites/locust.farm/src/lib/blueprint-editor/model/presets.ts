// The six ways of working the page offers, each backed by one of Locust's
// bundled examples. The sentences are the page's own plain wording.

import contract from '../../../../../../docs/reference/generated/organization.contract.json' with { type: 'json' };
import type { Blueprint } from '../contract/types.ts';

export interface WayOfWorking {
	/** The Locust example name. */
	id: string;
	title: string;
	sentence: string;
}

export const WAYS_OF_WORKING: readonly WayOfWorking[] = [
	{
		id: 'open',
		title: 'Open',
		sentence: 'Everyone works freely and shares what they find. You say when your own work is done.'
	},
	{
		id: 'coordinator',
		title: 'Coordinator',
		sentence: 'One person hands out work and accepts the results.'
	},
	{
		id: 'peer-review',
		title: 'Peer review',
		sentence: 'Anyone can work on a task. Someone else has to approve it before it counts.'
	},
	{
		id: 'review-panel',
		title: 'Review panel',
		sentence: 'Several reviewers look at each result. It counts once enough of them approve.'
	},
	{
		id: 'independent-attempts',
		title: 'Independent attempts',
		sentence:
			'Several people try the same task in their own way. A judge picks the result to use, and the other attempts are kept.'
	},
	{
		id: 'pipeline',
		title: 'Pipeline',
		sentence: 'Work moves through steps in order. Each step starts when the one before it is done.'
	}
];

export const DEFAULT_WAY = 'open';

const EXAMPLES = new Map(
	contract.examples.map((example) => [example.name, example.blueprint as unknown as Blueprint])
);

/** A fresh copy of a way of working's blueprint. */
export function presetBlueprint(id: string): Blueprint {
	const blueprint = EXAMPLES.get(id);
	if (!blueprint) throw new Error(`Unknown way of working: ${id}`);
	return structuredClone(blueprint);
}

export function wayOfWorking(id: string | null): WayOfWorking | undefined {
	return WAYS_OF_WORKING.find((way) => way.id === id);
}
