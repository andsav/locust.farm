import { error } from '@sveltejs/kit';
import { articleFor, manifest } from '#lib/docs/content.ts';
export const entries = () => manifest.pages.map(({ slug }) => ({ file: `${slug}.md` }));
export const GET = ({ params }: { params: { file: string } }) => {
	const article = params.file.endsWith('.md') ? articleFor(params.file.slice(0, -3)) : undefined;
	if (!article) error(404, 'Documentation article not found');
	return new Response(article.raw, { headers: { 'Content-Type': 'text/markdown; charset=utf-8' } });
};

export const prerender = true;
