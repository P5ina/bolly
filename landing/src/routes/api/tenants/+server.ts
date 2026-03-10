import { json, error } from '@sveltejs/kit';
import type { RequestHandler } from './$types.js';
import { getTenantsByUser, provisionTenant } from '$lib/server/tenants.js';

// GET /api/tenants — list user's tenants
export const GET: RequestHandler = async ({ locals }) => {
	if (!locals.user) error(401, 'Not authenticated');
	const list = await getTenantsByUser(locals.user.id);
	return json(list);
};

// POST /api/tenants — provision new tenant
export const POST: RequestHandler = async ({ request, locals }) => {
	if (!locals.user) error(401, 'Not authenticated');

	const { slug, plan } = await request.json();

	if (!slug || !/^[a-z0-9][a-z0-9-]{1,30}[a-z0-9]$/.test(slug)) {
		error(400, 'Invalid slug. Use lowercase letters, numbers, and hyphens (3-32 chars).');
	}

	if (!['starter', 'companion', 'unlimited'].includes(plan)) {
		error(400, 'Invalid plan');
	}

	try {
		const tenant = await provisionTenant({
			userId: locals.user.id,
			slug,
			plan,
		});
		return json(tenant, { status: 201 });
	} catch (err: any) {
		if (err.message?.includes('unique') || err.code === '23505') {
			error(409, 'That slug is already taken');
		}
		throw err;
	}
};
