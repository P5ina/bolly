import { json } from '@sveltejs/kit';
import type { RequestHandler } from './$types.js';
import { authenticateTenant } from '$lib/server/internal-auth.js';

export const GET: RequestHandler = async ({ request }) => {
	const tenant = await authenticateTenant(request);
	return json({ plan: tenant.plan });
};
