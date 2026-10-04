import { manifest, pageUrl } from '#lib/docs/content.ts';
export const GET = () =>
	new Response(
		`<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">${['/', '/start', '/blueprints', '/docs', ...manifest.pages.map((page) => pageUrl(page.slug)), ...manifest.routes.map((route) => pageUrl(route.slug))].map((path) => `<url><loc>https://locust.farm${path}</loc></url>`).join('')}</urlset>`,
		{ headers: { 'Content-Type': 'application/xml; charset=utf-8' } }
	);

export const prerender = true;
