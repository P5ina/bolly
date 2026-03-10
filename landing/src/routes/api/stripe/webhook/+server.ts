import { error, json } from '@sveltejs/kit';
import type { RequestHandler } from './$types.js';
import { env } from '$env/dynamic/private';
import { stripe } from '$lib/server/stripe/index.js';
import { destroyTenant, provisionTenant } from '$lib/server/tenants.js';
import { db } from '$lib/server/db/index.js';
import { tenants } from '$lib/server/db/schema.js';
import { eq } from 'drizzle-orm';
import type { PlanId } from '$lib/server/stripe/index.js';

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
		case 'checkout.session.completed': {
			const session = event.data.object;
			const { user_id, slug, plan } = session.metadata ?? {};

			if (!user_id || !slug || !plan) {
				console.error('checkout.session.completed missing metadata:', session.metadata);
				break;
			}

			try {
				await provisionTenant({
					userId: user_id,
					slug,
					plan: plan as PlanId,
					stripeSubscriptionId: session.subscription as string,
				});
				console.log(`Provisioned tenant ${slug} for user ${user_id}`);
			} catch (err) {
				// Error is stored on the tenant record — user sees it on dashboard
				console.error(`Provisioning failed for ${slug}:`, err);
			}
			break;
		}

		case 'customer.subscription.deleted': {
			const subscription = event.data.object;
			const [tenant] = await db()
				.select()
				.from(tenants)
				.where(eq(tenants.stripeSubscriptionId, subscription.id))
				.limit(1);

			if (tenant) {
				await destroyTenant(tenant.id);
				console.log(`Destroyed tenant ${tenant.slug} (subscription cancelled)`);
			}
			break;
		}

		case 'customer.subscription.updated': {
			const subscription = event.data.object;
			if (subscription.status === 'past_due' || subscription.status === 'unpaid') {
				console.warn(`Subscription ${subscription.id} is ${subscription.status}`);
			}
			break;
		}
	}

	return json({ received: true });
};
