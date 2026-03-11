import Stripe from 'stripe';
import { env } from '$env/dynamic/private';
import { eq } from 'drizzle-orm';
import { db } from '$lib/server/db/index.js';
import { users } from '$lib/server/db/schema.js';

let _stripe: Stripe | null = null;

export function stripe(): Stripe {
	if (!_stripe) {
		_stripe = new Stripe(env.STRIPE_SECRET_KEY!, { apiVersion: '2025-02-24.acacia' });
	}
	return _stripe;
}

export const PLANS = {
	starter: {
		name: 'Starter',
		priceMonthly: 500, // cents
		storageLimit: 1024, // MB
		cpus: 1,
		memoryMb: 512,
		maxInstances: 1,
		messagesPerDay: 100,
		tokensPerMonth: 500_000,
	},
	companion: {
		name: 'Companion',
		priceMonthly: 1200,
		storageLimit: 5120,
		cpus: 1,
		memoryMb: 1024,
		maxInstances: 3,
		messagesPerDay: 300,
		tokensPerMonth: 2_000_000,
	},
	unlimited: {
		name: 'Unlimited',
		priceMonthly: 2500,
		storageLimit: 20480,
		cpus: 2,
		memoryMb: 4096,
		maxInstances: -1, // unlimited
		messagesPerDay: -1, // unlimited
		tokensPerMonth: -1, // unlimited
	},
} as const;

export type PlanId = keyof typeof PLANS;

export function priceIdForPlan(plan: PlanId): string {
	const map: Record<PlanId, string> = {
		starter: env.STRIPE_STARTER_PRICE_ID!,
		companion: env.STRIPE_COMPANION_PRICE_ID!,
		unlimited: env.STRIPE_UNLIMITED_PRICE_ID!,
	};
	return map[plan];
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
