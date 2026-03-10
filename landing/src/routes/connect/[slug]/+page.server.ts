import { redirect, error } from '@sveltejs/kit';
import type { PageServerLoad } from './$types.js';
import { getTenantBySlug, getTenantUrl } from '$lib/server/tenants.js';

export const load: PageServerLoad = async ({ locals, params, url }) => {
	const shareToken = url.searchParams.get('share');
	const tenant = await getTenantBySlug(params.slug);

	if (!tenant) error(404, 'Companion not found');
	if (!tenant.flyAppId) error(400, 'Companion not provisioned yet');

	// Allow access via share token (no login required)
	if (shareToken) {
		if (tenant.shareToken && shareToken === tenant.shareToken) {
			const tenantUrl = await getTenantUrl(tenant);
			redirect(302, `${tenantUrl}/auth?token=${encodeURIComponent(tenant.authToken!)}`);
		}
		error(403, 'Invalid share link');
	}

	// Owner access (requires login)
	if (!locals.user) redirect(302, '/login');
	if (tenant.userId !== locals.user.id) error(403, 'Not your companion');

	const tenantUrl = await getTenantUrl(tenant);
	redirect(302, `${tenantUrl}/auth?token=${encodeURIComponent(tenant.authToken!)}`);
};
