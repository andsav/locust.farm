import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, resolve, relative } from 'node:path';
import MarkdownIt from 'markdown-it';

const root = execFileSync('git', ['rev-parse', '--show-toplevel'], { encoding: 'utf8' }).trim();
export interface PageRecord {
	source: string;
	slug: string;
	title: string;
	description: string;
	group: string;
	order: number;
	audience: string[];
	status: string;
}
export interface ArtifactRecord {
	id: string;
	source: string;
	url: string;
	title: string;
	kind: string;
	status: string;
}
export interface Manifest {
	track: string;
	label: string;
	repository: string;
	versions: Record<string, string | number>;
	pages: PageRecord[];
	artifacts: ArtifactRecord[];
	sourceLinks: string[];
	routes: { slug: string; page: string; section: string | null }[];
}
export interface Heading {
	id: string;
	title: string;
	level: number;
}
export const manifest: Manifest = JSON.parse(readFileSync(resolve(root, 'docs/site.json'), 'utf8'));
export const sourceCommit = execFileSync('git', ['rev-parse', 'HEAD'], {
	cwd: root,
	encoding: 'utf8'
}).trim();
export const sourceDirty =
	execFileSync('git', ['status', '--porcelain'], { cwd: root, encoding: 'utf8' }).trim().length > 0;
export const pageUrl = (slug: string) => `/docs/next/${slug}`;
export const rawUrl = (slug: string) => `/docs/next/raw/${slug}.md`;

export function validateManifest(input: Manifest) {
	if (input.track !== 'next' || !input.label || !/^https:\/\/github\.com\//.test(input.repository))
		throw new Error('Invalid documentation track or repository');
	if (!Object.keys(input.versions).length) throw new Error('Missing contract versions');
	const slugs = new Set<string>();
	const sources = new Set<string>();
	for (const page of input.pages) {
		if (!/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(page.slug) || slugs.has(page.slug))
			throw new Error(`Invalid or duplicate slug: ${page.slug}`);
		if (
			!page.source.startsWith('docs/guide/') ||
			!page.source.endsWith('.md') ||
			page.source.includes('..') ||
			sources.has(page.source)
		)
			throw new Error(`Invalid or duplicate source: ${page.source}`);
		if (
			!page.title ||
			!page.description ||
			!page.group ||
			!page.audience.length ||
			!Number.isFinite(page.order) ||
			!['development', 'proposed', 'implemented', 'verified'].includes(page.status)
		)
			throw new Error(`Incomplete page metadata: ${page.slug}`);
		if (!existsSync(resolve(root, page.source))) throw new Error(`Missing article: ${page.source}`);
		slugs.add(page.slug);
		sources.add(page.source);
	}
	const artifactIds = new Set<string>();
	const urls = new Set<string>();
	for (const artifact of input.artifacts) {
		if (
			!artifact.id ||
			artifactIds.has(artifact.id) ||
			urls.has(artifact.url) ||
			!/^\/docs\/next\/(reference|examples)\/[a-z0-9.-]+\.json$/.test(artifact.url)
		)
			throw new Error(`Invalid or duplicate artifact: ${artifact.id}`);
		if (
			artifact.source.includes('..') ||
			!/^(docs\/reference|examples\/formations)\//.test(artifact.source) ||
			!existsSync(resolve(root, artifact.source))
		)
			throw new Error(`Invalid or missing artifact source: ${artifact.source}`);
		if (!artifact.title || !artifact.status || !artifact.kind)
			throw new Error(`Incomplete artifact metadata: ${artifact.id}`);
		artifactIds.add(artifact.id);
		urls.add(artifact.url);
	}
	for (const source of input.sourceLinks)
		if (source.includes('..') || !existsSync(resolve(root, source)))
			throw new Error(`Invalid approved source: ${source}`);
}
validateManifest(manifest);
manifest.pages.sort((a, b) => a.order - b.order);
export const articleEntries = () => [
	...manifest.pages.map(({ slug }) => ({ slug })),
	...manifest.routes.map(({ slug }) => ({ slug }))
];

export function headingId(text: string) {
	return (
		text
			.toLowerCase()
			.normalize('NFKD')
			.replace(/[^a-z0-9\s-]/g, '')
			.trim()
			.replace(/\s+/g, '-') || 'section'
	);
}
const headingParser = new MarkdownIt({ html: false });
function headingsFor(source: string): Heading[] {
	const counts = new Map<string, number>();
	const tokens = headingParser.parse(source, {});
	return tokens.flatMap((token, i) => {
		if (token.type !== 'heading_open') return [];
		const title = tokens[i + 1].content;
		const base = headingId(title);
		const count = (counts.get(base) ?? 0) + 1;
		counts.set(base, count);
		return [
			{ id: count === 1 ? base : `${base}-${count}`, title, level: Number(token.tag.slice(1)) }
		];
	});
}
export function resolveLink(href: string, source: string): string {
	if (/^(https?:|mailto:)/i.test(href)) return href;
	if (href.startsWith('//') || /^[a-z][a-z0-9+.-]*:/i.test(href))
		throw new Error(`Unsafe link: ${href}`);
	const [path, anchor] = href.split('#');
	const target = path
		? relative(root, resolve(root, dirname(source), decodeURIComponent(path)))
		: source;
	if (target.startsWith('..') || !existsSync(resolve(root, target)))
		throw new Error(`Missing or outside-repository link: ${href} in ${source}`);
	const article = manifest.pages.find((page) => page.source === target);
	if (article) {
		if (
			anchor &&
			!headingsFor(readFileSync(resolve(root, target), 'utf8')).some(
				(heading) => heading.id === anchor
			)
		)
			throw new Error(`Missing anchor: ${href} in ${source}`);
		return `${pageUrl(article.slug)}${anchor ? `#${anchor}` : ''}`;
	}
	const artifact = manifest.artifacts.find((artifact) => artifact.source === target);
	if (artifact) return artifact.url;
	// Evidence/source links require explicit publication review.
	if (!manifest.sourceLinks.includes(target)) throw new Error(`Unapproved source link: ${href}`);
	return `${manifest.repository}/blob/main/${target}${anchor ? `#${anchor}` : ''}`;
}
export function parseArticle(markdown: string, source: string) {
	const parser = new MarkdownIt({ html: false, linkify: false });
	const headings = headingsFor(markdown);
	let headingIndex = 0;
	const normalHeading = parser.renderer.rules.heading_open;
	parser.renderer.rules.heading_open = (tokens, index, options, env, self) => {
		tokens[index].attrSet('id', headings[headingIndex++].id);
		return normalHeading
			? normalHeading(tokens, index, options, env, self)
			: self.renderToken(tokens, index, options);
	};
	const tokens = parser.parse(markdown, {});
	let raw = markdown;
	for (const token of tokens)
		for (const child of token.children ?? []) {
			if (child.type !== 'link_open' && child.type !== 'image') continue;
			const attr = child.type === 'image' ? 'src' : 'href';
			const href = String(child.attrGet(attr)!);
			const mapped = resolveLink(href, source);
			child.attrSet(attr, mapped);
			// Canonical content uses ordinary inline Markdown links, also checked by the repository checker.
			raw = raw.replaceAll(`](${href})`, `](${mapped})`);
		}
	return {
		html: parser.renderer.render(tokens, parser.options, {}),
		headings,
		raw,
		text: tokens
			.filter((t) => t.type === 'inline' || t.type === 'fence')
			.map((t) => t.content)
			.join('\n')
	};
}
export function artifactFor(url: string) {
	const artifact = manifest.artifacts.find((artifact) => artifact.url === url);
	if (!artifact) return undefined;
	const bytes = readFileSync(resolve(root, artifact.source), 'utf8');
	return { ...artifact, bytes, sha256: createHash('sha256').update(bytes).digest('hex') };
}
const cell = (value: unknown) =>
	String(value ?? '')
		.replaceAll('|', '\\|')
		.replace(/\s+/g, ' ');
function generatedReference() {
	const schema = JSON.parse(artifactFor('/docs/next/reference/organization.schema.json')!.bytes);
	const contract = JSON.parse(
		artifactFor('/docs/next/reference/organization.contract.json')!.bytes
	);
	if (
		contract.schema_version !== manifest.versions.formationSchema ||
		JSON.stringify(schema) !== JSON.stringify(contract.schema)
	)
		throw new Error('Schema/contract/version export drift');
	const operations = contract.operations as {
		name: string;
		input: string;
		output: string;
		summary: string;
	}[];
	const examples = contract.examples as { name: string; description: string; formation: unknown }[];
	for (const example of examples) {
		const artifact = artifactFor(`/docs/next/examples/${example.name}.json`);
		if (
			!artifact ||
			JSON.stringify(JSON.parse(artifact.bytes)) !== JSON.stringify(example.formation)
		)
			throw new Error(`Example export drift: ${example.name}`);
	}
	const required: string[] = schema.required ?? [];
	const fields = Object.entries(
		schema.properties as Record<string, { description?: string; type?: string; $ref?: string }>
	);
	return (
		'\n\n## Offline CLI operations\n\n| Command | Input | Output | Purpose |\n| --- | --- | --- | --- |\n' +
		operations
			.map(
				(op) =>
					`| \`locust formation ${cell(op.name)}\` | ${cell(op.input)} | ${cell(op.output)} | ${cell(op.summary)} |`
			)
			.join('\n') +
		'\n\n## Formation root fields\n\n| Field | Required | Shape | Description |\n| --- | --- | --- | --- |\n' +
		fields
			.map(
				([name, field]) =>
					`| \`${cell(name)}\` | ${required.includes(name) ? 'yes' : 'defaulted/optional'} | ${cell(field.type ?? field.$ref ?? 'see schema')} | ${cell(field.description)} |`
			)
			.join('\n') +
		'\n\n## Checked example downloads\n\n' +
		examples
			.map(
				(example) =>
					`- [${cell(example.name)}](${manifest.artifacts.find((a) => a.id === `example-${example.name}`)!.source.replace('examples/', '../../examples/')}): ${cell(example.description)}`
			)
			.join('\n') +
		'\n'
	);
}
interface ContractSchema {
	properties?: Record<string, ContractSchema>;
	required?: string[];
	oneOf?: ContractSchema[];
	$defs?: Record<string, ContractSchema>;
	$ref?: string;
	description?: string;
	enum?: string[];
}
interface CommandContract {
	name: string;
	summary?: string;
	arguments: {
		name: string;
		long?: string;
		positional: boolean;
		required: boolean;
		help?: string;
	}[];
	commands: CommandContract[];
}
function generatedRuntimeReference() {
	const contract = JSON.parse(artifactFor('/docs/next/reference/runtime.contract.json')!.bytes) as {
		api_version: number;
		protocol_version: number;
		operations: {
			name: string;
			audience: string;
			read_only: boolean;
			mcp_tool: string | null;
			summary: string;
		}[];
		request_schema: ContractSchema;
		response_schema: ContractSchema;
		event_schema: ContractSchema;
		error_schema: ContractSchema;
		cli: CommandContract;
	};
	if (
		contract.api_version !== manifest.versions.api ||
		contract.protocol_version !== manifest.versions.protocol
	)
		throw new Error('Runtime contract/version export drift');
	const fields = (schema: ContractSchema) =>
		Object.keys(schema.properties ?? {})
			.map((name) => `\`${cell(name)}${schema.required?.includes(name) ? '*' : ''}\``)
			.join(', ') || 'none';
	const variants = (schema: ContractSchema) =>
		(schema.oneOf ?? []).flatMap(
			(branch) =>
				branch.enum?.map((name) => [name, 'none']) ??
				Object.entries(branch.properties ?? {}).map(([name, value]) => [
					name,
					value.$ref ? `\`${cell(value.$ref.replace('#/$defs/', ''))}\`` : fields(value)
				])
		);
	const requests = new Map(
		variants(contract.request_schema).map(([name, fields]) => [name, fields] as const)
	);
	const commands: string[] = [];
	function visit(command: CommandContract, path: string[]) {
		const name = [...path, command.name];
		if (!command.commands.length)
			commands.push(
				`| \`${cell(name.join(' '))}\` | ${
					command.arguments
						.map(
							(argument) =>
								`\`${cell(argument.positional ? `<${argument.name}>` : `--${argument.long}`)}${argument.required ? '*' : ''}\``
						)
						.join(', ') || 'none'
				} | ${cell(command.summary)} |`
			);
		for (const child of command.commands) visit(child, name);
	}
	visit(contract.cli, []);
	return (
		'\n\n## Generated local API and MCP operations\n\nAn asterisk marks a required field in the wire schema; nullable values can still be explicitly null. Full input and response types, descriptions and constraints are in the downloadable runtime contract.\n\n| Operation | Audience | Read only | MCP tool | Input fields |\n| --- | --- | --- | --- | --- |\n' +
		contract.operations
			.map(
				(operation) =>
					`| \`${cell(operation.name)}\` | ${cell(operation.audience)} | ${operation.read_only ? 'yes' : 'no'} | ${operation.mcp_tool ? `\`${cell(operation.mcp_tool)}\`` : 'not exposed'} | ${requests.get(operation.name) ?? 'see schema'} |`
			)
			.join('\n') +
		'\n\n## Generated CLI\n\nArguments come from the actual command builder. An asterisk marks a required argument. Global flags include `--home`, `--credential`, `--session`, `--owner`, `--as`, `--json` and `--idempotency-key`; their accepted combination depends on the command. Composite values use JSON.\n\n| Command | Arguments | Purpose |\n| --- | --- | --- |\n' +
		commands.join('\n') +
		'\n\n## Generated response variants\n\n| Variant | Shape |\n| --- | --- |\n' +
		variants(contract.response_schema)
			.map(([name, shape]) => `| \`${cell(name)}\` | ${shape} |`)
			.join('\n') +
		'\n\n## Generated signed event variants\n\n| Event | Body fields |\n| --- | --- |\n' +
		variants(contract.event_schema)
			.map(([name, shape]) => `| \`${cell(name)}\` | ${shape} |`)
			.join('\n') +
		'\n\n## Generated error codes\n\n' +
		(contract.error_schema.$defs?.ErrorCode.enum ?? [])
			.map((code) => `- \`${cell(code)}\``)
			.join('\n') +
		'\n'
	);
}
export function articleFor(slug: string) {
	const subject = manifest.routes.find((route) => route.slug === slug);
	const page = manifest.pages.find((page) => page.slug === (subject?.page ?? slug));
	if (!page) return undefined;
	const markdown =
		readFileSync(resolve(root, page.source), 'utf8') +
		(page.slug === 'schema-reference'
			? generatedReference()
			: page.slug === 'runtime-reference'
				? generatedRuntimeReference()
				: '');
	return {
		...page,
		...parseArticle(markdown, page.source),
		url: pageUrl(page.slug),
		rawUrl: rawUrl(page.slug),
		section: subject?.section ?? null,
		hash: createHash('sha256').update(markdown).digest('hex')
	};
}
export function inventory() {
	return {
		track: manifest.track,
		label: manifest.label,
		sourceCommit,
		sourceDirty,
		versions: manifest.versions,
		routes: manifest.routes.map((route) => ({
			...route,
			url: pageUrl(route.slug),
			canonicalUrl: pageUrl(route.page) + (route.section ? `#${route.section}` : ''),
			rawUrl: rawUrl(route.page)
		})),
		artifacts: manifest.artifacts.map((artifact) => {
			const output = artifactFor(artifact.url)!;
			return { ...artifact, sha256: output.sha256 };
		}),
		pages: manifest.pages.map((page) => {
			const article = articleFor(page.slug)!;
			return {
				...page,
				url: article.url,
				rawUrl: article.rawUrl,
				sha256: createHash('sha256').update(article.raw).digest('hex'),
				headings: article.headings,
				text: article.text
			};
		})
	};
}

export function validateSubjectRoutes() {
	const routes = new Set(manifest.pages.map((page) => page.slug));
	for (const route of manifest.routes) {
		if (!/^[a-z0-9-]+(?:\/[a-z0-9-]+)*$/.test(route.slug) || routes.has(route.slug))
			throw new Error(`Invalid or duplicate subject route: ${route.slug}`);
		const article = articleFor(route.page);
		if (
			!article ||
			(route.section && !article.headings.some((heading) => heading.id === route.section))
		)
			throw new Error(`Missing subject page/section: ${route.slug}`);
		routes.add(route.slug);
	}
}
validateSubjectRoutes();
