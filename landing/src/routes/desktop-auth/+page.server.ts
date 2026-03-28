import { redirect } from '@sveltejs/kit';
import type { PageServerLoad } from './$types.js';

export const load: PageServerLoad = async ({ locals }) => {
	if (!locals.user) redirect(302, '/login?redirect=/desktop-auth');
	if (!locals.sessionId) redirect(302, '/login?redirect=/desktop-auth');

	return { sessionId: locals.sessionId };
};
