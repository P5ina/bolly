import Stripe from 'stripe';
import { STRIPE_COMPANION_BYOK_PRICE_ID, STRIPE_COMPANION_PRICE_ID, STRIPE_SECRET_KEY, STRIPE_STARTER_BYOK_PRICE_ID, STRIPE_STARTER_PRICE_ID, STRIPE_UNLIMITED_BYOK_PRICE_ID, STRIPE_UNLIMITED_PRICE_ID } from '$env/static/private';
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
	starter: {
		name: 'Starter',
		priceMonthly: 1200, // cents
		storageLimit: 10240, // MB (10 GB)
		cpus: 1,
		memoryMb: 2048,
		maxInstances: 1,
		tokensPer4h: 100_000,
		tokensPerMonth: 1_000_000,
	},
	companion: {
		name: 'Companion',
		priceMonthly: 2900,
		storageLimit: 20480, // MB (20 GB)
		cpus: 2,
		memoryMb: 4096,
		maxInstances: 3,
		tokensPer4h: 200_000,
		tokensPerMonth: 3_000_000,
	},
	unlimited: {
		name: 'Real Friend',
		priceMonthly: 5900,
		storageLimit: 51200, // MB (50 GB)
		cpus: 4,
		memoryMb: 4096,
		maxInstances: -1, // unlimited
		tokensPer4h: 500_000,
		tokensPerMonth: 10_000_000,
	},
} as const;

export type PlanId = keyof typeof PLANS;

export function priceIdForPlan(plan: PlanId, byok = false): string {
	if (byok) {
		const map: Record<PlanId, string> = {
			starter: STRIPE_STARTER_BYOK_PRICE_ID,
			companion: STRIPE_COMPANION_BYOK_PRICE_ID,
			unlimited: STRIPE_UNLIMITED_BYOK_PRICE_ID,
		};
		return map[plan];
	}
	const map: Record<PlanId, string> = {
		starter: STRIPE_STARTER_PRICE_ID,
		companion: STRIPE_COMPANION_PRICE_ID,
		unlimited: STRIPE_UNLIMITED_PRICE_ID,
	};
	return map[plan];
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
