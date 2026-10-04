import assert from 'node:assert/strict';
import { test } from 'node:test';
import { inspect, inspectFormation } from '../contract/inspect.ts';
import { pageChecks, withoutHiddenCharacters } from './checks.ts';
import { formationText, presentation, readPresentation } from './document.ts';
import {
	addRole,
	applyWay,
	newDocument,
	removeRole,
	removeStage,
	renameRole,
	renameStage,
	stageOrder,
	wouldLoop
} from './edit.ts';
import { record, redo, startHistory, undo } from './history.ts';
import {
	addAnswer,
	addKind,
	addStep,
	afterAnswer,
	countsAnswer,
	kinds,
	lineRules,
	MAIN,
	pickAnswer,
	sameAsMain,
	setAdd,
	setAfter,
	setCounts,
	setPick,
	setWork,
	useMain,
	workAnswer,
	type LineRef
} from './line.ts';
import { matchingWay, WAYS_OF_WORKING } from './presets.ts';
import { phrase, summarize } from './words.ts';

const valid = (document: { formation: Parameters<typeof inspectFormation>[0] }) =>
	inspectFormation(document.formation).valid;

test('every way of working is a valid formation with Locust and with the page', () => {
	for (const way of WAYS_OF_WORKING) {
		const document = newDocument(way.id);
		assert.ok(valid(document), way.id);
		assert.ok(inspect(formationText(document.formation)).valid, way.id);
	}
});

test('the formation text round-trips through the strict loader', () => {
	for (const way of WAYS_OF_WORKING) {
		const text = formationText(newDocument(way.id).formation);
		assert.ok(inspect(text).valid);
		assert.equal(text.endsWith('\n'), false);
		assert.equal(text.includes('\r'), false);
	}
});

test('each way of working is four answers', () => {
	const answers = (id: string) => {
		const rules = lineRules(newDocument(id).formation, MAIN);
		return [
			addAnswer(rules.work).kind,
			workAnswer(rules.work).kind,
			countsAnswer(rules.decisions.completion),
			pickAnswer(rules.decisions).pick.kind
		];
	};
	assert.deepEqual(answers('open'), [
		'anyone',
		'anyone',
		{ kind: 'list', approvals: null, check: null },
		'nobody'
	]);
	assert.deepEqual(answers('coordinator'), [
		'anyone',
		'asks',
		{
			kind: 'list',
			approvals: { by: { kind: 'role', name: 'coordinator' }, count: 1, excludeAuthor: false },
			check: null
		},
		'role'
	]);
	assert.deepEqual(answers('review-panel')[2], {
		kind: 'list',
		approvals: { by: { kind: 'role', name: 'reviewer' }, count: 2, excludeAuthor: true },
		check: null
	});
	assert.equal(answers('independent-attempts')[3], 'role');
});

test('the lit card is the one whose rules match, whatever was clicked', () => {
	for (const way of WAYS_OF_WORKING) {
		assert.equal(matchingWay(newDocument(way.id).formation), way.id);
	}
	// Open with one approval is Peer review.
	let document = setCounts(newDocument('open'), MAIN, {
		kind: 'list',
		approvals: { by: { kind: 'anyone' }, count: 1, excludeAuthor: true },
		check: null
	});
	assert.equal(matchingWay(document.formation), 'peer-review');
	// A role's description is not a rule.
	document = addRole(newDocument('open'), 'judge');
	document = setPick(document, MAIN, { pick: { kind: 'role', name: 'judge' } });
	assert.equal(matchingWay(document.formation), 'independent-attempts');
	// An arrangement no card shows.
	document = setCounts(document, MAIN, {
		kind: 'list',
		approvals: { by: { kind: 'anyone' }, count: 2, excludeAuthor: true },
		check: { name: 'tests', by: { kind: 'anyone' } }
	});
	assert.equal(matchingWay(document.formation), null);
	assert.ok(valid(document));
});

test('approvals and a check combine, and are read back', () => {
	const answer = {
		kind: 'list' as const,
		approvals: { by: { kind: 'anyone' as const }, count: 1, excludeAuthor: true },
		check: { name: 'tests', by: { kind: 'anyone' as const } }
	};
	const document = setCounts(newDocument('open'), MAIN, answer);
	assert.equal(document.formation.decisions.completion.kind, 'all');
	assert.deepEqual(countsAnswer(document.formation.decisions.completion), answer);
	assert.ok(valid(document));
	assert.equal(
		phrase(document.formation, MAIN, 'counts'),
		'After 1 approval, not the author\'s and the check "tests" reported as passed.'
	);
	const none = setCounts(document, MAIN, { kind: 'list', approvals: null, check: null });
	assert.deepEqual(none.formation.decisions.completion, {
		kind: 'declaration',
		by: { kind: 'contribution_author' }
	});
});

test('who adds and who works are written as rules Locust enforces', () => {
	let document = addRole(newDocument('open'), 'builder');
	document = setWork(document, MAIN, { kind: 'role', name: 'builder' });
	assert.deepEqual(document.formation.work.publish, { kind: 'role', name: 'builder' });
	assert.deepEqual(workAnswer(document.formation.work), { kind: 'role', name: 'builder' });
	document = setWork(document, MAIN, { kind: 'asks', by: { kind: 'role', name: 'builder' } });
	assert.equal(workAnswer(document.formation.work).kind, 'asks');
	assert.deepEqual(document.formation.work.publish, { kind: 'members' });
	// No tasks: nobody adds them and nobody starts work.
	document = setAdd(document, MAIN, { kind: 'none' });
	assert.deepEqual(document.formation.work.starts, []);
	assert.equal(workAnswer(document.formation.work).kind, 'none');
	assert.ok(valid(document));
	// Tasks again: someone can work on them.
	document = setAdd(document, MAIN, { kind: 'anyone' });
	assert.equal(workAnswer(document.formation.work).kind, 'anyone');
});

test('the pipeline is two steps, and only the draft has an answer of its own', () => {
	const formation = newDocument('pipeline').formation;
	assert.deepEqual(stageOrder(formation), ['draft', 'ship']);
	const draft: LineRef = { kind: 'step', name: 'draft' };
	const ship: LineRef = { kind: 'step', name: 'ship' };
	assert.equal(sameAsMain(formation, draft, 'counts'), false);
	assert.equal(sameAsMain(formation, draft, 'work'), true);
	assert.equal(sameAsMain(formation, ship, 'counts'), true);
	assert.deepEqual(afterAnswer(formation, 'draft'), { kind: 'start' });
	assert.deepEqual(afterAnswer(formation, 'ship'), { kind: 'after', step: 'draft', picked: false });
	assert.equal(phrase(formation, draft, 'counts'), "After 1 approval, not the author's.");
	assert.equal(phrase(formation, ship, 'add'), 'Locust, after "draft" has a result that counts.');
	assert.deepEqual(kinds(formation), []);
});

test('a step gets rules of its own only where it differs, and gives them up again', () => {
	const added = addStep(newDocument('open'));
	let document = added.document;
	assert.equal(added.name, 'step');
	assert.equal(document.formation.flow.step.task_type, null);
	const step: LineRef = { kind: 'step', name: 'step' };
	const approvals = { by: { kind: 'anyone' as const }, count: 1, excludeAuthor: true };
	document = setCounts(document, step, { kind: 'list', approvals, check: null });
	assert.equal(document.formation.flow.step.task_type, 'step');
	assert.equal(document.formation.task_types.step.work, null);
	assert.equal(document.formation.decisions.completion.kind, 'declaration');
	assert.ok(valid(document));
	document = useMain(document, step, 'counts');
	assert.equal(document.formation.flow.step.task_type, null);
	assert.deepEqual(document.formation.task_types, {});
});

test('a step keeps following the main rules at the points it did not change', () => {
	let document = addRole(addStep(newDocument('open')).document, 'judge');
	const step: LineRef = { kind: 'step', name: 'step' };
	document = setCounts(document, step, {
		kind: 'list',
		approvals: { by: { kind: 'anyone' }, count: 1, excludeAuthor: true },
		check: null
	});
	// The picker is set later, for any task; the step follows it.
	document = setPick(document, MAIN, { pick: { kind: 'role', name: 'judge' } });
	assert.deepEqual(lineRules(document.formation, step).decisions.selection, {
		kind: 'role',
		name: 'judge'
	});
	assert.equal(sameAsMain(document.formation, step, 'pick'), true);
	assert.equal(sameAsMain(document.formation, step, 'counts'), false);
});

test('new steps wait for the one before; a later step waits for a pick when someone picks', () => {
	let document = addRole(newDocument('open'), 'judge');
	document = addStep(document).document;
	document = addStep(document).document;
	assert.deepEqual(stageOrder(document.formation), ['step', 'step 2']);
	assert.deepEqual(document.formation.flow['step 2'].requires, [
		{ stage: 'step', evidence: 'completion' }
	]);
	document = setPick(
		document,
		{ kind: 'step', name: 'step' },
		{ pick: { kind: 'role', name: 'judge' } }
	);
	assert.deepEqual(document.formation.flow['step 2'].requires, [
		{ stage: 'step', evidence: 'selection' }
	]);
	assert.ok(valid(document));
	document = setPick(document, { kind: 'step', name: 'step' }, { pick: { kind: 'nobody' } });
	assert.equal(document.formation.flow['step 2'].requires[0].evidence, 'completion');
	assert.ok(valid(document));
});

test('who a step is sent to follows who works on it', () => {
	let document = addRole(addStep(newDocument('open')).document, 'builder');
	const step: LineRef = { kind: 'step', name: 'step' };
	document = setWork(document, step, { kind: 'role', name: 'builder' });
	assert.deepEqual(document.formation.flow.step.recipients, { kind: 'role', name: 'builder' });
	document = useMain(document, step, 'work');
	assert.deepEqual(document.formation.flow.step.recipients, { kind: 'members' });
	assert.ok(valid(document));
});

test('renaming and removing steps keeps the order consistent', () => {
	let document = renameStage(newDocument('pipeline'), 'draft', 'write');
	assert.deepEqual(document.formation.flow.ship.requires, [
		{ stage: 'write', evidence: 'completion' }
	]);
	// The step's own rules follow its name.
	assert.equal(document.formation.flow.write.task_type, 'write');
	assert.ok(Object.hasOwn(document.formation.task_types, 'write'));
	assert.ok(valid(document));
	document = removeStage(document, 'write');
	assert.deepEqual(document.formation.flow.ship.requires, []);
	assert.deepEqual(document.formation.task_types, {});
	assert.ok(valid(document));
});

test('removing a step in the middle joins the steps around it', () => {
	let document = addStep(newDocument('pipeline')).document;
	assert.deepEqual(stageOrder(document.formation), ['draft', 'ship', 'step']);
	document = removeStage(document, 'ship');
	assert.deepEqual(document.formation.flow.step.requires, [
		{ stage: 'draft', evidence: 'completion' }
	]);
});

test('a step cannot wait for a step that waits for it', () => {
	const document = newDocument('pipeline');
	assert.equal(wouldLoop(document.formation, 'ship', 'draft'), true);
	assert.equal(setAfter(document, 'draft', 'ship'), document);
	assert.equal(wouldLoop(document.formation, 'draft', 'ship'), false);
	assert.deepEqual(setAfter(document, 'ship', null).formation.flow.ship.requires, []);
});

test('another kind of task follows the main rules until a point is changed', () => {
	const added = addKind(addRole(newDocument('open'), 'security'));
	let document = added.document;
	assert.equal(added.name, 'kind');
	const kind: LineRef = { kind: 'kind', name: 'kind' };
	assert.deepEqual(kinds(document.formation), ['kind']);
	assert.equal(sameAsMain(document.formation, kind, 'counts'), true);
	document = setCounts(document, kind, {
		kind: 'list',
		approvals: { by: { kind: 'role', name: 'security' }, count: 1, excludeAuthor: true },
		check: null
	});
	assert.equal(sameAsMain(document.formation, kind, 'counts'), false);
	assert.equal(document.formation.decisions.completion.kind, 'declaration');
	assert.ok(valid(document));
});

test('the name is the only presentation the page keeps, beside other tools keys', () => {
	const read = readPresentation({ polaris: { zoom: 2 }, 'locust.farm': { name: 'Sync' } });
	const document = { ...newDocument('pipeline'), others: read.others, name: read.name ?? '' };
	assert.deepEqual(presentation(document), {
		polaris: { zoom: 2 },
		'locust.farm': { name: 'Sync' }
	});
});

test('renaming a role rewrites every rule that names it', () => {
	const document = renameRole(newDocument('coordinator'), 'coordinator', 'lead');
	const text = formationText(document.formation);
	assert.equal(text.includes('"coordinator"'), false);
	assert.ok(valid(document));
	assert.deepEqual(Object.keys(document.formation.roles), ['lead']);
});

test('removing a role leaves its rules to nobody, never to everyone', () => {
	const document = removeRole(newDocument('coordinator'), 'coordinator');
	assert.equal(document.formation.decisions.selection, null);
	assert.equal(document.formation.decisions.finish, null);
	const completion = document.formation.decisions.completion;
	assert.equal(completion.kind === 'reviews' && completion.by.kind, 'nobody');
	assert.equal(valid(document), false);
});

test('the summary is plain', () => {
	const lines = summarize(newDocument('open').formation).map((line) => line.text);
	assert.deepEqual(lines, [
		'There are no roles. Every member takes part on equal terms.',
		'Any member can add tasks.',
		'Any member can work on a task. There is no lock: two members can work on the same task.',
		'A result counts when its author says so.',
		'Nobody picks one result. Every result that counts stays.'
	]);
	const pipeline = summarize(newDocument('pipeline').formation).map((line) => line.text);
	assert.ok(
		pipeline.includes(
			'Step "draft": Locust adds this task at the start. A result counts when it has 1 approval, not the author\'s.'
		)
	);
	assert.ok(
		pipeline.includes(
			'Step "ship": Locust adds this task after "draft" has a result that counts. It follows the rules for any task.'
		)
	);
	for (const way of WAYS_OF_WORKING) {
		for (const line of summarize(newDocument(way.id).formation)) {
			assert.doesNotMatch(
				line.text,
				/variation|closure|materiali|selector|predicate|runner|stage/i,
				way.id
			);
		}
	}
});

test('undo and redo walk the history', () => {
	let history = startHistory(newDocument('open'));
	history = record(history, applyWay(history.present, 'pipeline'));
	history = record(history, setAdd(history.present, MAIN, { kind: 'none' }));
	assert.equal(history.past.length, 2);
	history = undo(history);
	assert.equal(matchingWay(history.present.formation), 'pipeline');
	history = undo(history);
	assert.equal(matchingWay(history.present.formation), 'open');
	history = redo(history);
	assert.equal(matchingWay(history.present.formation), 'pipeline');
	assert.equal(record(history, history.present), history);
});

test('hidden characters block copying and can be removed', () => {
	const document = addRole(newDocument('open'), 'lead\u{202e}');
	const checks = pageChecks(document.formation);
	assert.ok(checks.some((check) => check.blocksCopy && check.path === '/roles/lead\u{202e}'));
	assert.equal(withoutHiddenCharacters('lead\u{202e}\u{200b}'), 'lead');
});

test('serialization escapes characters that could hide or break lines', () => {
	const document = addRole(newDocument('open'), 'a\u{2028}b\u{E0041}');
	const text = formationText(document.formation);
	assert.equal(text.includes('\u{2028}'), false);
	assert.ok(text.includes('\\u2028'));
	assert.ok(text.includes('\\udb40\\udc41'));
	const parsed = JSON.parse(text);
	assert.ok(Object.hasOwn(parsed.roles, 'a\u{2028}b\u{E0041}'));
});

test('a reopened way of working is still that way of working', async () => {
	const { blocks } = await import('../prompt/prompt.ts');
	const { openText } = await import('../prompt/open.ts');
	for (const way of WAYS_OF_WORKING) {
		const opened = openText((await blocks(newDocument(way.id))).text);
		assert.ok(opened.ok);
		if (opened.ok) assert.equal(matchingWay(opened.document.formation), way.id, way.id);
	}
});
