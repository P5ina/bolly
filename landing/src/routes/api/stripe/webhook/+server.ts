import { error, json } from '@sveltejs/kit';
import type { RequestHandler } from './$types.js';
import { env } from '$env/dynamic/private';
import { stripe } from '$lib/server/stripe/index.js';
import { db } from '$lib/server/db/index.js';
import { tenants, users } from '$lib/server/db/schema.js';
import { eq } from 'drizzle-orm';
import { destroyTenant } from '$lib/server/tenants.js';

export const POST: RequestHandler = async ({ request }) => {
	const body = await request.text();
	const sig = request.headers.get('stripe-signature');

	if (!sig) error(400, 'Missing signature');

	let event;
	try {
		event = stripe().webhooks.constructEvent(body, sig, env.STRIPE_WEBHOOK_SECRET!);
	} catch {
		error(400, 'Invalid signature');
	}

	switch (event.type) {
		case 'customer.subscription.deleted': {
			const subscription = event.data.object;
			// Find and destroy tenant with this subscription
			const [tenant] = await db()
				.select()
				.from(tenants)
				.where(eq(tenants.stripeSubscriptionId, subscription.id))
				.limit(1);

			if (tenant) {
				await destroyTenant(tenant.id);
			}
			break;
		}

		case 'customer.subscription.updated': {
			const subscription = event.data.object;
			if (subscription.status === 'past_due' || subscription.status === 'unpaid') {
				// Could stop machine here to save costs
				// For now just log
				console.warn(`Subscription ${subscription.id} is ${subscription.status}`);
			}
			break;
		}
	}

	return json({ received: true });
};
