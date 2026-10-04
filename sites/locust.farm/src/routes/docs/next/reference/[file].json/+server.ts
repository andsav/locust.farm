import { error } from '@sveltejs/kit';
import { artifactFor, manifest } from '#lib/docs/content.ts';
export const entries = () =>
	manifest.artifacts
		.filter((artifact) => artifact.url.startsWith('/docs/next/reference/'))
		.map((artifact) => ({ file: artifact.url.split('/').at(-1)!.slice(0, -5) }));
export const GET = ({ params }: { params: { file: string } }) => {
	const artifact = artifactFor(`/docs/next/reference/${params.file}.json`);
	if (!artifact) error(404, 'Documentation artifact not found');
	return new Response(artifact.bytes, {
		headers: { 'Content-Type': 'application/json; charset=utf-8' }
	});
};

export const prerender = true;
