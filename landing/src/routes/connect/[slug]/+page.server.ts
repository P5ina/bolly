import { redirect, error } from '@sveltejs/kit';
import type { PageServerLoad } from './$types.js';
import { getTenantBySlug } from '$lib/server/tenants.js';

export const load: PageServerLoad = async ({ locals, params }) => {
	if (!locals.user) redirect(302, '/login');

	const tenant = await getTenantBySlug(params.slug);

	if (!tenant) error(404, 'Companion not found');
	if (tenant.userId !== locals.user.id) error(403, 'Not your companion');
	if (!tenant.flyAppId) error(400, 'Companion not provisioned yet');

	const url = `https://${tenant.flyAppId}.fly.dev/auth?token=${encodeURIComponent(tenant.authToken!)}`;
	redirect(302, url);
};
