import { redirect } from '@sveltejs/kit';
import type { PageServerLoad } from './$types.js';
import { getTenantsByUser } from '$lib/server/tenants.js';

export const load: PageServerLoad = async ({ locals }) => {
	if (!locals.user) redirect(302, '/login');

	const tenantsList = await getTenantsByUser(locals.user.id);

	return {
		user: {
			id: locals.user.id,
			email: locals.user.email,
			name: locals.user.name,
		},
		tenants: tenantsList.map((t) => ({
			id: t.id,
			slug: t.slug,
			plan: t.plan,
			status: t.status,
			flyAppId: t.flyAppId,
			createdAt: t.createdAt.toISOString(),
		})),
	};
};
