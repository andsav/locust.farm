import { llmsText } from '#lib/onboarding/llms.ts';

export const prerender = true;

export function GET() {
	return new Response(llmsText(), {
		headers: { 'content-type': 'text/plain; charset=utf-8' }
	});
}
