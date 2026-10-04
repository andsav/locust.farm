// The exported Rust schema is the only source for public snapshot TypeScript.
import { readFile, writeFile } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';
import { format, resolveConfig } from 'prettier';

export const schemaPath = new URL(
	'../../../docs/reference/generated/farm.schema.json',
	import.meta.url
);
export const targetPath = new URL('../src/lib/farm/types.ts', import.meta.url);

/** @typedef {boolean | { $ref?: string, title?: string, $defs?: Record<string, Schema>, type?: string | string[], const?: unknown, enum?: unknown[], anyOf?: Schema[], oneOf?: Schema[], allOf?: Schema[], items?: Schema, properties?: Record<string, Schema>, required?: string[], additionalProperties?: Schema }} Schema */
/** @param {Schema} schema @returns {string} */
function typeOf(schema) {
	if (schema === true) return 'unknown';
	if (schema === false) return 'never';
	if (schema.$ref) return schema.$ref.split('/').at(-1) ?? 'never';
	if ('const' in schema) return JSON.stringify(schema.const) ?? 'never';
	if (schema.enum) return schema.enum.map((value) => JSON.stringify(value)).join(' | ');
	if (schema.anyOf || schema.oneOf)
		return (schema.anyOf ?? schema.oneOf ?? []).map(typeOf).join(' | ');
	if (schema.allOf)
		return schema.allOf
			.map(typeOf)
			.map((type) => `(${type})`)
			.join(' & ');
	if (Array.isArray(schema.type))
		return schema.type.map((type) => typeOf({ ...schema, type })).join(' | ');
	switch (schema.type) {
		case 'string':
			return 'string';
		case 'integer':
		case 'number':
			return 'number';
		case 'boolean':
			return 'boolean';
		case 'null':
			return 'null';
		case 'array': {
			if (schema.items === undefined) throw new Error('Array schema requires items');
			return `Array<${typeOf(schema.items)}>`;
		}
		case 'object': {
			if (!schema.properties)
				return `Record<string, ${typeOf(schema.additionalProperties ?? true)}>`;
			const required = new Set(schema.required ?? []);
			const fields = Object.entries(schema.properties).map(
				([name, value]) =>
					`${JSON.stringify(name)}${required.has(name) ? '' : '?'}: ${typeOf(value)};`
			);
			if (schema.additionalProperties) {
				throw new Error('Public farm records must not allow additional properties');
			}
			return `{ ${fields.join('\n')} }`;
		}
		default:
			throw new Error(`Unsupported public schema shape: ${JSON.stringify(schema)}`);
	}
}

export async function generateFarmTypes() {
	/** @type {Schema} */
	const schema = JSON.parse(await readFile(schemaPath, 'utf8'));
	if (typeof schema !== 'object' || !schema.title)
		throw new Error('Expected a named object schema');
	const definitions = Object.entries(schema.$defs ?? {}).sort(([a], [b]) => a.localeCompare(b));
	const names = [schema.title, ...definitions.map(([name]) => name)];
	if (names.some((name) => !/^[A-Za-z][A-Za-z0-9_]*$/.test(name)))
		throw new Error('Invalid schema type name');
	const source =
		'// Generated from Rust FarmSnapshot by scripts/generate-farm-types.mjs. Do not edit.\n' +
		`export type ${schema.title} = ${typeOf(schema)};\n` +
		definitions.map(([name, value]) => `export type ${name} = ${typeOf(value)};`).join('\n');
	return format(source, {
		...(await resolveConfig(targetPath.pathname)),
		parser: 'typescript',
		plugins: []
	});
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
	const expected = await generateFarmTypes();
	if (process.argv.includes('--write')) await writeFile(targetPath, expected);
	else if ((await readFile(targetPath, 'utf8')) !== expected) {
		throw new Error('Farm type drift: run node scripts/generate-farm-types.mjs --write');
	}
}
