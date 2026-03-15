import { error } from '@sveltejs/kit';
import { db } from './db/index.js';
import { tenants, type Tenant } from './db/schema.js';
import { eq } from 'drizzle-orm';

/** Extract and validate Bearer token from request, return the tenant. */
export async function authenticateTenant(request: Request): Promise<Tenant> {
	const auth = request.headers.get('authorization');
	if (!auth?.startsWith('Bearer ')) {
		throw error(401, 'Missing auth token');
	}
	const authToken = auth.slice(7);

	const [tenant] = await db()
		.select()
		.from(tenants)
		.where(eq(tenants.authToken, authToken))
		.limit(1);

	if (!tenant) {
		throw error(401, 'Invalid auth token');
	}

	return tenant;
}
