// The six ways of working the page offers, each backed by one of Locust's
// bundled examples. The sentences are the page's own plain wording.

import contract from '../../../../../../docs/reference/generated/organization.contract.json' with { type: 'json' };
import type { Formation } from '../contract/types.ts';
import { formationData } from './document.ts';

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
		sentence: 'One member asks others to do tasks and approves the results.'
	},
	{
		id: 'peer-review',
		title: 'Peer review',
		sentence: 'Anyone can work on a task. Someone else has to approve a result before it counts.'
	},
	{
		id: 'review-panel',
		title: 'Review panel',
		sentence: 'Two reviewers have to approve each result before it counts.'
	},
	{
		id: 'independent-attempts',
		title: 'Independent attempts',
		sentence:
			'Several members try the same task in their own way. A judge picks the result to use, and the other results are kept.'
	},
	{
		id: 'pipeline',
		title: 'Pipeline',
		sentence:
			'Tasks that Locust adds in order. Each one is added when the one before it has a result that counts.'
	}
];

export const DEFAULT_WAY = 'open';

const EXAMPLES = new Map(
	contract.examples.map((example) => [example.name, example.formation as unknown as Formation])
);

/** A fresh copy of a way of working's formation. */
export function presetFormation(id: string): Formation {
	const formation = EXAMPLES.get(id);
	if (!formation) throw new Error(`Unknown way of working: ${id}`);
	return structuredClone(formation);
}

export function wayOfWorking(id: string | null): WayOfWorking | undefined {
	return WAYS_OF_WORKING.find((way) => way.id === id);
}

/** The rules alone: role descriptions are words about a role, not rules. */
function rules(formation: Formation): string {
	const data = formationData(formation);
	return JSON.stringify({ ...data, roles: Object.keys(data.roles as object) });
}

const RULES = new Map(WAYS_OF_WORKING.map((way) => [rules(presetFormation(way.id)), way.id]));

/** The way of working whose rules equal this formation's, if any. */
export function matchingWay(formation: Formation): string | null {
	return RULES.get(rules(formation)) ?? null;
}
