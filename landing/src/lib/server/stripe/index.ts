import Stripe from 'stripe';
import { env } from '$env/dynamic/private';

let _stripe: Stripe | null = null;

export function stripe(): Stripe {
	if (!_stripe) {
		_stripe = new Stripe(env.STRIPE_SECRET_KEY!, { apiVersion: '2025-04-30.basil' });
	}
	return _stripe;
}

export const PLANS = {
	starter: {
		name: 'Starter',
		priceMonthly: 500, // cents
		storageLimit: 1024, // MB
		maxInstances: 1,
	},
	companion: {
		name: 'Companion',
		priceMonthly: 1200,
		storageLimit: 5120,
		maxInstances: 3,
	},
	unlimited: {
		name: 'Unlimited',
		priceMonthly: 2500,
		storageLimit: 20480,
		maxInstances: -1, // unlimited
	},
} as const;

export type PlanId = keyof typeof PLANS;

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
	});
	return session.url!;
}

export async function createCustomer(email: string, name?: string): Promise<string> {
	const customer = await stripe().customers.create({ email, name: name ?? undefined });
	return customer.id;
}

export async function createBillingPortalSession(customerId: string, returnUrl: string): Promise<string> {
	const session = await stripe().billingPortal.sessions.create({
		customer: customerId,
		return_url: returnUrl,
	});
	return session.url;
}
