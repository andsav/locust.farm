import assert from 'node:assert/strict';
import { test } from 'node:test';
import { inspect, inspectBlueprint } from '../contract/inspect.ts';
import { doneAnswer, setDoneAnswer, setStartAnswer, startAnswer } from './answers.ts';
import { pageChecks, withoutHiddenCharacters } from './checks.ts';
import { blueprintText, presentation, readPresentation } from './document.ts';
import {
	addRequirement,
	addRole,
	addStage,
	applyWay,
	moveStage,
	newDocument,
	removeRole,
	removeStage,
	renameRole,
	renameStage,
	setStageCompletion,
	setWork,
	stageOrder,
	wouldLoop
} from './edit.ts';
import { record, redo, startHistory, undo } from './history.ts';
import { WAYS_OF_WORKING } from './presets.ts';
import { summarize } from './words.ts';

test('every way of working is a valid blueprint with Locust and with the page', () => {
	for (const way of WAYS_OF_WORKING) {
		const document = newDocument(way.id);
		assert.ok(inspectBlueprint(document.blueprint).valid, way.id);
		assert.ok(inspect(blueprintText(document.blueprint)).valid, way.id);
	}
});

test('the blueprint text round-trips through the strict loader', () => {
	for (const way of WAYS_OF_WORKING) {
		const document = newDocument(way.id);
		const text = blueprintText(document.blueprint);
		const result = inspect(text);
		assert.ok(result.valid);
		assert.equal(text.endsWith('\n'), false);
		assert.equal(text.includes('\r'), false);
	}
});

test('moving stages changes only the layout', () => {
	const document = newDocument('pipeline');
	const before = blueprintText(document.blueprint);
	const moved = moveStage(document, 'draft', { x: 400, y: 120 });
	assert.equal(blueprintText(moved.blueprint), before);
	assert.deepEqual(presentation(moved)['locust.farm'], {
		name: '',
		way: 'pipeline',
		stages: { draft: { x: 400, y: 120 } }
	});
});

test('the presentation keeps other tools keys and drops positions of missing stages', () => {
	const read = readPresentation({
		polaris: { zoom: 2 },
		'locust.farm': { name: 'Sync', stages: { draft: { x: 1, y: 2 }, gone: { x: 0, y: 0 } } }
	});
	const document = { ...newDocument('pipeline'), layout: read.layout, name: read.name ?? '' };
	const out = presentation(document);
	assert.deepEqual(out.polaris, { zoom: 2 });
	assert.deepEqual(out['locust.farm'], {
		name: 'Sync',
		way: 'pipeline',
		stages: { draft: { x: 1, y: 2 } }
	});
});

test('renaming a role rewrites every rule that names it', () => {
	const document = renameRole(newDocument('coordinator'), 'coordinator', 'lead');
	const text = blueprintText(document.blueprint);
	assert.equal(text.includes('"coordinator"'), false);
	assert.ok(inspectBlueprint(document.blueprint).valid);
	assert.deepEqual(Object.keys(document.blueprint.roles), ['lead']);
});

test('removing a role leaves its rules to nobody, never to everyone', () => {
	const document = removeRole(newDocument('coordinator'), 'coordinator');
	assert.equal(document.blueprint.decisions.selection, null);
	assert.equal(document.blueprint.decisions.finish, null);
	const completion = document.blueprint.decisions.completion;
	assert.equal(completion.kind === 'reviews' && completion.by.kind, 'nobody');
	assert.equal(inspectBlueprint(document.blueprint).valid, false);
});

test('renaming and removing stages keeps prerequisites consistent', () => {
	let document = moveStage(newDocument('pipeline'), 'draft', { x: 0, y: 0 });
	document = renameStage(document, 'draft', 'write');
	assert.deepEqual(document.blueprint.flow.review.requires, [
		{ stage: 'write', evidence: 'completion' }
	]);
	assert.deepEqual(document.layout.stages, { write: { x: 0, y: 0 } });
	assert.ok(inspectBlueprint(document.blueprint).valid);
	document = removeStage(document, 'write');
	assert.deepEqual(document.blueprint.flow.review.requires, []);
	assert.ok(inspectBlueprint(document.blueprint).valid);
});

test('a connection that would make a stage wait for itself is refused', () => {
	const document = newDocument('pipeline');
	assert.equal(wouldLoop(document.blueprint, 'review', 'draft'), true);
	assert.equal(addRequirement(document, 'review', 'draft', 'completion'), document);
	assert.equal(wouldLoop(document.blueprint, 'draft', 'review'), false);
});

test('new stages get a runner role and start in order', () => {
	let document = newDocument('open');
	document = addStage(document, 'design', { x: 0, y: 0 });
	document = addStage(document, 'build', { x: 300, y: 0 });
	document = addRequirement(document, 'design', 'build', 'completion');
	assert.ok(Object.hasOwn(document.blueprint.roles, 'runner'));
	assert.deepEqual(stageOrder(document), ['design', 'build']);
	assert.ok(inspectBlueprint(document.blueprint).valid);
});

test('a stage with its own done rule gets a task type named after it', () => {
	let document = addStage(newDocument('open'), 'review', null);
	document = setStageCompletion(document, 'review', {
		kind: 'reviews',
		by: { kind: 'members' },
		count: 1,
		exclude_author: true
	});
	assert.equal(document.blueprint.flow.review.task_type, 'review');
	assert.equal(document.blueprint.task_types.review.decisions?.completion.kind, 'reviews');
	assert.ok(inspectBlueprint(document.blueprint).valid);
	document = setStageCompletion(document, 'review', null);
	assert.equal(document.blueprint.flow.review.task_type, null);
	assert.equal(Object.hasOwn(document.blueprint.task_types, 'review'), false);
});

test('answers are read back from the blueprint', () => {
	assert.deepEqual(startAnswer(newDocument('open').blueprint.work.starts), { kind: 'anyone' });
	assert.equal(startAnswer(newDocument('coordinator').blueprint.work.starts).kind, 'handed-out');
	assert.deepEqual(doneAnswer(newDocument('open').blueprint.decisions.completion), {
		kind: 'self'
	});
	const review = doneAnswer(newDocument('peer-review').blueprint.decisions.completion);
	assert.equal(review.kind, 'review');
	let document = setStartAnswer(newDocument('open'), { kind: 'nobody' });
	assert.deepEqual(document.blueprint.work.starts, []);
	document = setDoneAnswer(document, {
		kind: 'review',
		by: { kind: 'members' },
		count: 2,
		excludeAuthor: true
	});
	assert.equal(doneAnswer(document.blueprint.decisions.completion).kind, 'review');
});

test('the summary is plain', () => {
	const lines = summarize(newDocument('open')).map((line) => line.text);
	assert.deepEqual(lines, [
		'Everyone in the goal takes part on equal terms.',
		'Anyone in the goal can suggest tasks and share findings.',
		'Anyone in the goal can start working on a task. Several people may work on the same task.',
		'A task is done when the person who did it says so.',
		'Every result that is done is kept. Nobody picks a single final answer.',
		'Nobody can say the goal is finished. It stays open.'
	]);
	for (const way of WAYS_OF_WORKING) {
		for (const line of summarize(newDocument(way.id))) {
			assert.doesNotMatch(line.text, /variation|closure|materiali|selector|predicate/i, way.id);
		}
	}
});

test('undo and redo walk the history', () => {
	let history = startHistory(newDocument('open'));
	history = record(history, applyWay(history.present, 'pipeline'));
	history = record(history, setWork(history.present, { starts: [] }));
	assert.equal(history.past.length, 2);
	history = undo(history);
	assert.equal(history.present.way, 'pipeline');
	history = undo(history);
	assert.equal(history.present.way, 'open');
	history = redo(history);
	assert.equal(history.present.way, 'pipeline');
	assert.equal(record(history, history.present), history);
});

test('hidden characters block copying and can be removed', () => {
	const document = addRole(newDocument('open'), 'lead\u{202e}');
	const checks = pageChecks(document.blueprint);
	assert.ok(checks.some((check) => check.blocksCopy && check.path === '/roles/lead\u{202e}'));
	assert.equal(withoutHiddenCharacters('lead\u{202e}\u{200b}'), 'lead');
});

test('serialization escapes characters that could hide or break lines', () => {
	const document = addRole(newDocument('open'), 'a\u{2028}b\u{E0041}');
	const text = blueprintText(document.blueprint);
	assert.equal(text.includes('\u{2028}'), false);
	assert.ok(text.includes('\\u2028'));
	assert.ok(text.includes('\\udb40\\udc41'));
	const parsed = JSON.parse(text);
	assert.ok(Object.hasOwn(parsed.roles, 'a\u{2028}b\u{E0041}'));
});

test('a reopened way of working shows no changes', async () => {
	const { blocks } = await import('../prompt/prompt.ts');
	const { openText } = await import('../prompt/open.ts');
	const { changedFromWay } = await import('./answers.ts');
	for (const way of WAYS_OF_WORKING) {
		const document = newDocument(way.id);
		const opened = openText((await blocks(document)).text);
		assert.ok(opened.ok);
		if (opened.ok)
			assert.deepEqual(changedFromWay(opened.document, newDocument(way.id)), [], way.id);
	}
});
