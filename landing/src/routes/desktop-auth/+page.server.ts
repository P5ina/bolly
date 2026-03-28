import { redirect } from '@sveltejs/kit';
import type { PageServerLoad } from './$types.js';

export const load: PageServerLoad = async ({ locals }) => {
	if (!locals.user) redirect(302, '/login?redirect=/desktop-auth');
	if (!locals.sessionId) redirect(302, '/login?redirect=/desktop-auth');

	// User is authenticated — redirect to desktop app via deep link
	redirect(302, `bolly://callback?session=${encodeURIComponent(locals.sessionId)}`);
};
