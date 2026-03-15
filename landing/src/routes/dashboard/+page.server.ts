import { fail, redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types.js';
import type Stripe from 'stripe';
import { getTenantsByUser, getTenantBySlug, provisionTenant, switchTenantChannel } from '$lib/server/tenants.js';
import { invalidateSession, deleteSessionCookie } from '$lib/server/auth/index.js';
import { stripe, createBillingPortalSession, ensureCustomer, PLANS, priceIdForPlan, swapSubscriptionPrice, type PlanId } from '$lib/server/stripe/index.js';
import { db } from '$lib/server/db/index.js';
import { tenants } from '$lib/server/db/schema.js';
import { eq } from 'drizzle-orm';
import * as fly from '$lib/server/fly/index.js';
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
					byok: t.byokApiKey ? {
						provider: t.byokProvider ?? 'anthropic',
						model: t.byokModel,
						keyHint: `...${t.byokApiKey.slice(-4)}`,
					} : null,
				};
			})
		),
		// All active subscriptions from Stripe
		ensureCustomer(locals.user)
			.then((cid) =>
				stripe()
					.subscriptions.list({
						customer: cid,
						status: 'active',
						expand: ['data.items.data.price.product'],
					})
					.then((res) => res.data)
			)
			.catch(() => [] as Stripe.Subscription[]),
	]);

	// Find orphaned subscriptions (active in Stripe but not linked to any tenant)
	const orphanedSubs = allSubscriptions.filter((sub) => !linkedSubIds.has(sub.id));

	// Auto-reconcile: provision tenants for orphaned subscriptions that have valid metadata
	for (const sub of orphanedSubs) {
		const { user_id, slug, plan } = sub.metadata ?? {};
		if (user_id === locals.user.id && slug && plan && ['starter', 'companion', 'unlimited'].includes(plan)) {
			try {
				console.log(`Auto-reconciling orphaned subscription ${sub.id} → ${slug}`);
				const tenant = await provisionTenant({
					userId: user_id,
					slug,
					plan: plan as PlanId,
					stripeSubscriptionId: sub.id,
				});
				// Add to the tenants list so it shows up immediately
				const planConfig = PLANS[plan as PlanId];
				tenantsWithSub.push({
					id: tenant.id,
					slug: tenant.slug,
					plan: tenant.plan,
					planName: planConfig?.name ?? tenant.plan,
					priceMonthly: planConfig?.priceMonthly ?? 0,
					status: tenant.status,
					flyAppId: tenant.flyAppId,
					errorMessage: tenant.errorMessage,
					imageChannel: tenant.imageChannel,
					shareToken: tenant.shareToken,
					createdAt: tenant.createdAt.toISOString(),
					subscription: await getSubscriptionInfo(sub.id),
					byok: null,
				});
				linkedSubIds.add(sub.id);
			} catch (err) {
				console.error(`Auto-reconcile failed for ${slug}:`, err);
			}
		}
	}

	const orphanedSubscriptions = orphanedSubs
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
		if (!locals.user) redirect(302, '/dashboard');

		const customerId = await ensureCustomer(locals.user);
		const returnUrl = `${env.ORIGIN ?? url.origin}/dashboard`;
		const portalUrl = await createBillingPortalSession(customerId, returnUrl);
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
		if (!locals.user) redirect(302, '/dashboard');

		const formData = await request.formData();
		const subscriptionId = formData.get('subscriptionId') as string;
		if (!subscriptionId) return fail(400, { error: 'Missing subscription ID' });

		try {
			const customerId = await ensureCustomer(locals.user);
			const sub = await stripe().subscriptions.retrieve(subscriptionId);
			if (sub.customer !== customerId) {
				return fail(403, { error: 'Not your subscription' });
			}
			await stripe().subscriptions.update(subscriptionId, {
				cancel_at_period_end: true,
			});
		} catch {
			return fail(500, { error: 'Failed to cancel subscription' });
		}
	},
	saveBYOK: async ({ request, locals }) => {
		if (!locals.user) redirect(302, '/login');

		const formData = await request.formData();
		const tenantId = formData.get('tenantId') as string;
		const provider = formData.get('provider') as string;
		const apiKey = formData.get('apiKey') as string;
		const model = (formData.get('model') as string) || null;

		if (!tenantId || !provider || !apiKey) return fail(400, { error: 'Missing required fields' });
		if (!['anthropic', 'openai', 'openrouter'].includes(provider)) return fail(400, { error: 'Invalid provider' });

		// Verify ownership
		const userTenants = await getTenantsByUser(locals.user.id);
		const tenant = userTenants.find((t) => t.id === tenantId);
		if (!tenant) return fail(403, { error: 'Not your companion' });
		if (tenant.status !== 'running') return fail(400, { error: 'Companion is not running' });

		// Validate the API key by making a test call
		try {
			await validateApiKey(provider, apiKey, model);
		} catch (err) {
			return fail(400, { error: `Invalid API key: ${err instanceof Error ? err.message : 'validation failed'}` });
		}

		// Save BYOK to DB
		await db()
			.update(tenants)
			.set({
				byokProvider: provider,
				byokApiKey: apiKey,
				byokModel: model,
				updatedAt: new Date(),
			})
			.where(eq(tenants.id, tenantId));

		// Push env vars to Fly machine
		if (tenant.flyAppId && tenant.flyMachineId) {
			const envPatch: Record<string, string> = {
				BOLLY_BYOK: 'true',
				BOLLY_LLM_PROVIDER: provider,
			};

			// Set the appropriate API key env var
			if (provider === 'anthropic') {
				envPatch.ANTHROPIC_API_KEY = apiKey;
			} else if (provider === 'openai') {
				envPatch.OPENAI_API_KEY = apiKey;
			} else if (provider === 'openrouter') {
				envPatch.OPENROUTER_API_KEY = apiKey;
			}

			if (model) {
				envPatch.BOLLY_LLM_MODEL = model;
			}

			try {
				await fly.updateMachineEnv(tenant.flyAppId, tenant.flyMachineId, envPatch);
			} catch (err) {
				console.error('Failed to update Fly env for BYOK:', err);
				return fail(500, { error: 'Saved key but failed to update companion. Try again.' });
			}
		}

		// Swap Stripe subscription to BYOK price
		if (tenant.stripeSubscriptionId) {
			try {
				const byokPriceId = priceIdForPlan(tenant.plan as PlanId, true);
				await swapSubscriptionPrice(tenant.stripeSubscriptionId, byokPriceId);
			} catch (err) {
				console.error('Failed to swap subscription to BYOK price:', err);
			}
		}
	},
	removeBYOK: async ({ request, locals }) => {
		if (!locals.user) redirect(302, '/login');

		const formData = await request.formData();
		const tenantId = formData.get('tenantId') as string;
		if (!tenantId) return fail(400, { error: 'Missing tenant ID' });

		const userTenants = await getTenantsByUser(locals.user.id);
		const tenant = userTenants.find((t) => t.id === tenantId);
		if (!tenant) return fail(403, { error: 'Not your companion' });

		// Clear BYOK in DB
		await db()
			.update(tenants)
			.set({
				byokProvider: null,
				byokApiKey: null,
				byokModel: null,
				updatedAt: new Date(),
			})
			.where(eq(tenants.id, tenantId));

		// Restore platform keys on Fly machine
		if (tenant.flyAppId && tenant.flyMachineId) {
			try {
				await fly.updateMachineEnv(tenant.flyAppId, tenant.flyMachineId, {
					BOLLY_BYOK: '',
					BOLLY_LLM_PROVIDER: '',
					BOLLY_LLM_MODEL: '',
					ANTHROPIC_API_KEY: env.ANTHROPIC_API_KEY ?? '',
					OPENAI_API_KEY: env.OPENAI_API_KEY ?? '',
					OPENROUTER_API_KEY: env.OPENROUTER_API_KEY ?? '',
				});
			} catch (err) {
				console.error('Failed to restore Fly env after BYOK removal:', err);
				return fail(500, { error: 'Removed key but failed to update companion. Try again.' });
			}
		}

		// Swap Stripe subscription back to normal price
		if (tenant.stripeSubscriptionId) {
			try {
				const normalPriceId = priceIdForPlan(tenant.plan as PlanId, false);
				await swapSubscriptionPrice(tenant.stripeSubscriptionId, normalPriceId);
			} catch (err) {
				console.error('Failed to swap subscription back to normal price:', err);
			}
		}
	},
};

/** Validate an API key by making a minimal test call. */
async function validateApiKey(provider: string, apiKey: string, model?: string | null): Promise<void> {
	if (provider === 'anthropic') {
		const res = await fetch('https://api.anthropic.com/v1/messages', {
			method: 'POST',
			headers: {
				'x-api-key': apiKey,
				'anthropic-version': '2023-06-01',
				'content-type': 'application/json',
			},
			body: JSON.stringify({
				model: model || 'claude-haiku-4-5-20251001',
				max_tokens: 1,
				messages: [{ role: 'user', content: 'hi' }],
			}),
		});
		if (!res.ok) {
			const text = await res.text();
			if (res.status === 401) throw new Error('Invalid API key');
			if (res.status === 403) throw new Error('API key does not have access');
			throw new Error(`API returned ${res.status}: ${text.slice(0, 200)}`);
		}
	} else {
		// OpenAI / OpenRouter
		const baseUrl = provider === 'openrouter'
			? 'https://openrouter.ai/api/v1'
			: 'https://api.openai.com/v1';
		const res = await fetch(`${baseUrl}/chat/completions`, {
			method: 'POST',
			headers: {
				Authorization: `Bearer ${apiKey}`,
				'Content-Type': 'application/json',
			},
			body: JSON.stringify({
				model: model || (provider === 'openrouter' ? 'anthropic/claude-haiku-4-5-20251001' : 'gpt-4o-mini'),
				max_tokens: 1,
				messages: [{ role: 'user', content: 'hi' }],
			}),
		});
		if (!res.ok) {
			const text = await res.text();
			if (res.status === 401) throw new Error('Invalid API key');
			throw new Error(`API returned ${res.status}: ${text.slice(0, 200)}`);
		}
	}
}
