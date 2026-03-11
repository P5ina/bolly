import { json, error } from '@sveltejs/kit';
import type { RequestHandler } from './$types.js';
import { getTenantsByUser, getTenantBySlug } from '$lib/server/tenants.js';
import { createCheckoutSession, ensureCustomer, priceIdForPlan, type PlanId } from '$lib/server/stripe/index.js';
import { env } from '$env/dynamic/private';

// GET /api/tenants — list user's tenants
export const GET: RequestHandler = async ({ locals }) => {
	if (!locals.user) error(401, 'Not authenticated');
	const list = await getTenantsByUser(locals.user.id);
	return json(list);
};

// POST /api/tenants — create Stripe checkout session, provision happens in webhook
export const POST: RequestHandler = async ({ request, locals }) => {
	if (!locals.user) error(401, 'Not authenticated');

	const { slug, plan } = await request.json();

	if (!slug || !/^[a-z0-9][a-z0-9-]{1,30}[a-z0-9]$/.test(slug)) {
		error(400, 'Invalid slug. Use lowercase letters, numbers, and hyphens (3-32 chars).');
	}

	if (!['starter', 'companion', 'unlimited'].includes(plan)) {
		error(400, 'Invalid plan');
	}

	const existing = await getTenantBySlug(slug);
	if (existing && existing.userId !== locals.user.id) {
		error(409, 'That name is already taken. Please choose another.');
	}

	const origin = env.ORIGIN ?? 'https://bollyai.dev';

	try {
		const customerId = await ensureCustomer(locals.user);
		const checkoutUrl = await createCheckoutSession({
			customerId,
			priceId: priceIdForPlan(plan as PlanId),
			successUrl: `${origin}/dashboard?checkout=success`,
			cancelUrl: `${origin}/dashboard?checkout=cancelled`,
			metadata: {
				user_id: locals.user.id,
				slug,
				plan,
			},
		});

		return json({ checkoutUrl });
	} catch (err: any) {
		console.error('Stripe checkout error:', err);
		error(500, err.message ?? 'Failed to create checkout session');
	}
};
