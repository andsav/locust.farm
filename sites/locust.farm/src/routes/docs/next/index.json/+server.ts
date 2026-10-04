import { json } from '@sveltejs/kit';
import { inventory } from '#lib/docs/content.ts';
export const GET = () => json(inventory());

export const prerender = true;
