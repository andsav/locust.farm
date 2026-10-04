import { error } from '@sveltejs/kit';
import {
	articleFor,
	articleEntries,
	manifest,
	sourceCommit,
	sourceDirty
} from '#lib/docs/content.ts';
export const entries = articleEntries;
export const load = ({ params }: { params: { slug: string } }) => {
	const article = articleFor(params.slug);
	if (!article) error(404, 'Documentation article not found');
	return { article, manifest, sourceCommit, sourceDirty };
};
