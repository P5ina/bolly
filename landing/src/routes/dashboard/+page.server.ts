import { fail, redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types.js';
import type Stripe from 'stripe';
import { getTenantsByUser, getTenantBySlug, provisionTenant, switchTenantChannel } from '$lib/server/tenants.js';
import { invalidateSession, deleteSessionCookie } from '$lib/server/auth/index.js';
import { stripe, createBillingPortalSession, PLANS, type PlanId } from '$lib/server/stripe/index.js';
import { env } from '$env/dynamic/private';

type SubscriptionInfo = {
	id: string;
	status: string;
	currentPeriodEnd: string;
	cancelAtPeriodEnd: boolean;
	amount: number;
	productName: string | null;
};

async function getSubscriptionInfo(subId: string): Promise<SubscriptionInfo | null> {
	try {
		const sub = await stripe().subscriptions.retrieve(subId, { expand: ['items.data.price.product'] });
		const item = sub.items.data[0];
		const price = item?.price;
		const product = price?.product;
		return {
			id: sub.id,
			status: sub.status,
			currentPeriodEnd: new Date(sub.current_period_end * 1000).toISOString(),
			cancelAtPeriodEnd: sub.cancel_at_period_end,
			amount: price?.unit_amount ?? 0,
			productName: typeof product === 'object' && product && 'name' in product ? product.name : null,
		};
	} catch {
		return null;
	}
}

export const load: PageServerLoad = async ({ locals }) => {
	if (!locals.user) redirect(302, '/login');

	const tenantsList = await getTenantsByUser(locals.user.id);

	// Fetch all Stripe subscriptions + tenant sub details in parallel
	const linkedSubIds = new Set(tenantsList.map((t) => t.stripeSubscriptionId).filter(Boolean));

	const [tenantsWithSub, allSubscriptions] = await Promise.all([
		// Tenant subscription details
		Promise.all(
			tenantsList.map(async (t) => {
				const subscription = t.stripeSubscriptionId
					? await getSubscriptionInfo(t.stripeSubscriptionId)
					: null;
				const planConfig = PLANS[t.plan as PlanId];
				return {
					id: t.id,
					slug: t.slug,
					plan: t.plan,
					planName: planConfig?.name ?? t.plan,
					priceMonthly: planConfig?.priceMonthly ?? 0,
					status: t.status,
					flyAppId: t.flyAppId,
					errorMessage: t.errorMessage,
					imageChannel: t.imageChannel,
					shareToken: t.shareToken,
					createdAt: t.createdAt.toISOString(),
					subscription,
				};
			})
		),
		// All active subscriptions from Stripe
		locals.user.stripeCustomerId
			? stripe()
					.subscriptions.list({
						customer: locals.user.stripeCustomerId,
						status: 'active',
						expand: ['data.items.data.price.product'],
					})
					.then((res) => res.data)
					.catch(() => [] as Stripe.Subscription[])
			: ([] as Stripe.Subscription[]),
	]);

	// Find orphaned subscriptions (active in Stripe but not linked to any tenant)
	const orphanedSubscriptions = allSubscriptions
		.filter((sub) => !linkedSubIds.has(sub.id))
		.map((sub) => {
			const item = sub.items.data[0];
			const price = item?.price;
			const product = price?.product;
			return {
				id: sub.id,
				status: sub.status,
				currentPeriodEnd: new Date(sub.current_period_end * 1000).toISOString(),
				cancelAtPeriodEnd: sub.cancel_at_period_end,
				amount: price?.unit_amount ?? 0,
				productName: typeof product === 'object' && product && 'name' in product ? product.name : null,
				metadata: sub.metadata,
			};
		});

	return {
		user: {
			id: locals.user.id,
			email: locals.user.email,
			name: locals.user.name,
		},
		tenants: tenantsWithSub,
		orphanedSubscriptions,
	};
};

export const actions: Actions = {
	logout: async ({ locals, cookies }) => {
		if (locals.sessionId) {
			await invalidateSession(locals.sessionId);
		}
		deleteSessionCookie(cookies);
		redirect(302, '/');
	},
	billing: async ({ locals, url }) => {
		if (!locals.user?.stripeCustomerId) redirect(302, '/dashboard');

		const returnUrl = `${env.ORIGIN ?? url.origin}/dashboard`;
		const portalUrl = await createBillingPortalSession(locals.user.stripeCustomerId, returnUrl);
		redirect(303, portalUrl);
	},
	retryProvision: async ({ request, locals }) => {
		if (!locals.user) redirect(302, '/login');

		const formData = await request.formData();
		const slug = formData.get('slug') as string;
		if (!slug) return fail(400, { error: 'Missing slug' });

		const tenant = await getTenantBySlug(slug);
		if (!tenant) return fail(404, { error: 'Tenant not found' });
		if (tenant.userId !== locals.user.id) return fail(403, { error: 'Not your companion' });
		if (tenant.status !== 'error') return fail(400, { error: 'Tenant is not in error state' });

		try {
			await provisionTenant({
				userId: locals.user.id,
				slug,
				plan: tenant.plan as PlanId,
				stripeSubscriptionId: tenant.stripeSubscriptionId ?? undefined,
			});
		} catch {
			return fail(500, { error: 'Provisioning failed again. Please contact support.' });
		}
	},
	switchChannel: async ({ request, locals }) => {
		if (!locals.user) redirect(302, '/login');

		const formData = await request.formData();
		const tenantId = formData.get('tenantId') as string;
		const channel = formData.get('channel') as string;

		if (!tenantId || !channel) return fail(400, { error: 'Missing fields' });
		if (channel !== 'stable' && channel !== 'nightly') return fail(400, { error: 'Invalid channel' });

		const tenant = await getTenantBySlug(
			(await getTenantsByUser(locals.user.id)).find((t) => t.id === tenantId)?.slug ?? ''
		);
		if (!tenant || tenant.userId !== locals.user.id) return fail(403, { error: 'Not your companion' });
		if (tenant.status !== 'running') return fail(400, { error: 'Tenant is not running' });

		try {
			await switchTenantChannel(tenantId, channel);
		} catch {
			return fail(500, { error: 'Failed to switch channel' });
		}
	},
	cancelSubscription: async ({ request, locals }) => {
		if (!locals.user?.stripeCustomerId) redirect(302, '/dashboard');

		const formData = await request.formData();
		const subscriptionId = formData.get('subscriptionId') as string;
		if (!subscriptionId) return fail(400, { error: 'Missing subscription ID' });

		try {
			// Verify this subscription belongs to the user
			const sub = await stripe().subscriptions.retrieve(subscriptionId);
			if (sub.customer !== locals.user.stripeCustomerId) {
				return fail(403, { error: 'Not your subscription' });
			}
			await stripe().subscriptions.update(subscriptionId, {
				cancel_at_period_end: true,
			});
		} catch {
			return fail(500, { error: 'Failed to cancel subscription' });
		}
	},
};
