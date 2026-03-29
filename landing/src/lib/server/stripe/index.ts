import Stripe from 'stripe';
import { STRIPE_SECRET_KEY } from '$env/static/private';
import { env } from '$env/dynamic/private';
import { eq } from 'drizzle-orm';
import { db } from '$lib/server/db/index.js';
import { users } from '$lib/server/db/schema.js';

let _stripe: Stripe | null = null;

export function stripe(): Stripe {
	if (!_stripe) {
		_stripe = new Stripe(STRIPE_SECRET_KEY, { apiVersion: '2025-02-24.acacia' });
	}
	return _stripe;
}

export const PLANS = {
	companion: {
		name: 'Companion',
		priceMonthly: 1000, // cents ($10/mo BYOK)
		storageLimit: 20480, // MB (20 GB)
		cpus: 2,
		memoryMb: 4096,
		maxInstances: 3,
	},
} as const;

export type PlanId = keyof typeof PLANS;

export function priceIdForPlan(_plan: PlanId, _byok = false): string {
	return env.STRIPE_COMPANION_BYOK_PRICE_ID ?? env.STRIPE_COMPANION_PRICE_ID ?? '';
}

/** Swap a subscription's price (e.g. normal → BYOK or back). */
export async function swapSubscriptionPrice(subscriptionId: string, newPriceId: string): Promise<void> {
	const sub = await stripe().subscriptions.retrieve(subscriptionId);
	const item = sub.items.data[0];
	if (!item) throw new Error('Subscription has no items');
	await stripe().subscriptions.update(subscriptionId, {
		items: [{ id: item.id, price: newPriceId }],
		proration_behavior: 'create_prorations',
	});
}

export async function createCheckoutSession(opts: {
	customerId: string;
	priceId: string;
	successUrl: string;
	cancelUrl: string;
	metadata?: Record<string, string>;
}): Promise<string> {
	const session = await stripe().checkout.sessions.create({
		customer: opts.customerId,
		mode: 'subscription',
		line_items: [{ price: opts.priceId, quantity: 1 }],
		success_url: opts.successUrl,
		cancel_url: opts.cancelUrl,
		metadata: opts.metadata,
		subscription_data: {
			metadata: opts.metadata,
		},
	});
	return session.url!;
}

export async function createCustomer(email: string, name?: string): Promise<string> {
	const customer = await stripe().customers.create({ email, name: name ?? undefined });
	return customer.id;
}

/**
 * Ensure a user has a valid Stripe customer ID.
 * If the stored ID is missing or invalid (e.g. test-mode ID in prod), create a new one.
 */
export async function ensureCustomer(user: { id: string; email: string; name?: string | null; stripeCustomerId?: string | null }): Promise<string> {
	if (user.stripeCustomerId) {
		try {
			await stripe().customers.retrieve(user.stripeCustomerId);
			return user.stripeCustomerId;
		} catch {
			// Customer doesn't exist (wrong mode or deleted) — create a new one
			console.warn(`Stripe customer ${user.stripeCustomerId} not found, creating new one for ${user.email}`);
		}
	}

	const customerId = await createCustomer(user.email, user.name ?? undefined);

	// Update DB
	await db().update(users).set({ stripeCustomerId: customerId }).where(eq(users.id, user.id));

	return customerId;
}

export async function createBillingPortalSession(customerId: string, returnUrl: string): Promise<string> {
	const session = await stripe().billingPortal.sessions.create({
		customer: customerId,
		return_url: returnUrl,
	});
	return session.url;
}
