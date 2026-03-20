import { error, fail, redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types.js';
import { db } from '$lib/server/db/index.js';
import { tenants, users, rateLimits } from '$lib/server/db/schema.js';
import { eq, ne } from 'drizzle-orm';
import { env } from '$env/dynamic/private';
import * as fly from '$lib/server/fly/index.js';
import { provisionTenant } from '$lib/server/tenants.js';
import { PLANS, type PlanId, stripe, priceIdForPlan } from '$lib/server/stripe/index.js';
import { sendPriceChangeEmail } from '$lib/server/email/index.js';

function isAdmin(email?: string): boolean {
	if (!email) return false;
	const list = (env.ADMIN_EMAILS ?? '').split(',').map((e) => e.trim().toLowerCase());
	return list.includes(email.toLowerCase());
}

export const load: PageServerLoad = async ({ locals }) => {
	if (!locals.user) redirect(302, '/login');
	if (!isAdmin(locals.user.email)) error(403, 'Forbidden');

	const allTenants = await db()
		.select({
			id: tenants.id,
			slug: tenants.slug,
			plan: tenants.plan,
			status: tenants.status,
			flyAppId: tenants.flyAppId,
			flyMachineId: tenants.flyMachineId,
			imageChannel: tenants.imageChannel,
			storageLimit: tenants.storageLimit,
			tokensPerMonth: tenants.tokensPerMonth,
			errorMessage: tenants.errorMessage,
			createdAt: tenants.createdAt,
			updatedAt: tenants.updatedAt,
			userId: tenants.userId,
			userEmail: users.email,
			userName: users.name,
		})
		.from(tenants)
		.innerJoin(users, eq(tenants.userId, users.id))
		.orderBy(tenants.createdAt);

	const allRateLimits = await db().select().from(rateLimits);
	const rateLimitMap = Object.fromEntries(allRateLimits.map((r) => [r.instanceId, r]));

	// Fetch current machine env for running tenants to get active model
	const machineEnvs = await Promise.all(
		allTenants
			.filter((t) => t.status === 'running' && t.flyAppId && t.flyMachineId)
			.map(async (t) => {
				try {
					const machine = await fly.getMachine(t.flyAppId!, t.flyMachineId!);
					return [t.id, {
						state: machine.state,
						provider: machine.config?.env?.BOLLY_LLM_PROVIDER ?? null,
						model: machine.config?.env?.BOLLY_LLM_MODEL ?? null,
						fastModel: machine.config?.env?.BOLLY_LLM_FAST_MODEL ?? null,
					}] as const;
				} catch {
					return [t.id, null] as const;
				}
			}),
	);
	const machineMap = Object.fromEntries(machineEnvs);

	const allUsers = await db()
		.select({ id: users.id, email: users.email, name: users.name })
		.from(users)
		.orderBy(users.email);

	return {
		users: allUsers,
		tenants: allTenants.map((t) => ({
			...t,
			createdAt: t.createdAt.toISOString(),
			updatedAt: t.updatedAt.toISOString(),
			rateLimit: rateLimitMap[t.id] ?? null,
			machine: machineMap[t.id] ?? null,
		})),
	};
};

export const actions: Actions = {
	updateModel: async ({ request, locals }) => {
		if (!locals.user || !isAdmin(locals.user.email)) error(403, 'Forbidden');

		const form = await request.formData();
		const tenantId = form.get('tenantId') as string;
		const provider = form.get('provider') as string;
		const model = form.get('model') as string;
		const fastModel = (form.get('fastModel') as string) ?? '';

		if (!tenantId || !provider || !model) return fail(400, { error: 'Missing fields' });

		const [tenant] = await db()
			.select()
			.from(tenants)
			.where(eq(tenants.id, tenantId))
			.limit(1);

		if (!tenant || !tenant.flyAppId || !tenant.flyMachineId) {
			return fail(400, { error: 'Tenant not found or not running' });
		}

		try {
			await fly.updateMachineEnv(tenant.flyAppId, tenant.flyMachineId, {
				BOLLY_LLM_PROVIDER: provider,
				BOLLY_LLM_MODEL: model,
				...(fastModel ? { BOLLY_LLM_FAST_MODEL: fastModel } : {}),
			});
		} catch (e) {
			const msg = e instanceof Error ? e.message : 'Unknown error';
			return fail(500, { error: `Failed to update machine: ${msg}` });
		}

		return { success: true, tenantId };
	},

	stopMachine: async ({ request, locals }) => {
		if (!locals.user || !isAdmin(locals.user.email)) error(403, 'Forbidden');

		const form = await request.formData();
		const tenantId = form.get('tenantId') as string;

		const [tenant] = await db()
			.select()
			.from(tenants)
			.where(eq(tenants.id, tenantId))
			.limit(1);

		if (!tenant || !tenant.flyAppId || !tenant.flyMachineId) {
			return fail(400, { error: 'Tenant not found or not running' });
		}

		try {
			await fly.stopMachine(tenant.flyAppId, tenant.flyMachineId);
			await db()
				.update(tenants)
				.set({ status: 'stopped', updatedAt: new Date() })
				.where(eq(tenants.id, tenantId));
		} catch (e) {
			const msg = e instanceof Error ? e.message : 'Unknown error';
			return fail(500, { error: `Failed to stop machine: ${msg}` });
		}
	},

	startMachine: async ({ request, locals }) => {
		if (!locals.user || !isAdmin(locals.user.email)) error(403, 'Forbidden');

		const form = await request.formData();
		const tenantId = form.get('tenantId') as string;

		const [tenant] = await db()
			.select()
			.from(tenants)
			.where(eq(tenants.id, tenantId))
			.limit(1);

		if (!tenant || !tenant.flyAppId || !tenant.flyMachineId) {
			return fail(400, { error: 'Tenant not found or not running' });
		}

		try {
			await fly.startMachine(tenant.flyAppId, tenant.flyMachineId);
			await db()
				.update(tenants)
				.set({ status: 'running', updatedAt: new Date() })
				.where(eq(tenants.id, tenantId));
		} catch (e) {
			const msg = e instanceof Error ? e.message : 'Unknown error';
			return fail(500, { error: `Failed to start machine: ${msg}` });
		}
	},

	updateAllImages: async ({ locals }) => {
		if (!locals.user || !isAdmin(locals.user.email)) error(403, 'Forbidden');

		const allTenants = await db()
			.select({
				id: tenants.id,
				slug: tenants.slug,
				flyAppId: tenants.flyAppId,
				flyMachineId: tenants.flyMachineId,
				imageChannel: tenants.imageChannel,
				status: tenants.status,
				byokProvider: tenants.byokProvider,
			})
			.from(tenants)
			.where(eq(tenants.status, 'running'));

		let updated = 0;
		const errors: string[] = [];

		for (const t of allTenants) {
			if (!t.flyAppId || !t.flyMachineId) continue;
			try {
				const image = fly.imageForChannel((t.imageChannel as fly.ImageChannel) ?? 'stable');
				// Single atomic update: image + env together to avoid race conditions.
				// updateMachineEnv then updateMachineImage can lose env vars if the
				// GET in updateMachineImage returns stale config.
				await fly.updateMachineImageAndEnv(t.flyAppId, t.flyMachineId, image, {
					BOLLY_RELEASE_TOKEN: env.BOLLY_RELEASE_TOKEN ?? '',
					GOOGLE_CLIENT_ID: env.GOOGLE_CLIENT_ID ?? '',
					GOOGLE_CLIENT_SECRET: env.GOOGLE_CLIENT_SECRET ?? '',
					...(t.byokProvider ? {} : {
						ANTHROPIC_API_KEY: env.ANTHROPIC_API_KEY ?? '',
						OPENAI_API_KEY: env.OPENAI_API_KEY ?? '',
						OPENROUTER_API_KEY: env.OPENROUTER_API_KEY ?? '',
						BRAVE_SEARCH_API_KEY: env.BRAVE_SEARCH_API_KEY ?? '',
						ELEVENLABS_API_KEY: env.ELEVENLABS_API_KEY ?? '',
					}),
				});
				updated++;
			} catch (e) {
				const msg = e instanceof Error ? e.message : 'Unknown error';
				errors.push(`${t.slug}: ${msg}`);
			}
		}

		if (errors.length > 0) {
			return fail(500, { error: `Updated ${updated}, failed ${errors.length}: ${errors.join('; ')}` });
		}

		return { success: true, updated };
	},

	syncPlanLimits: async ({ locals }) => {
		if (!locals.user || !isAdmin(locals.user.email)) error(403, 'Forbidden');

		const allTenants = await db()
			.select({ id: tenants.id, plan: tenants.plan })
			.from(tenants)
			.where(ne(tenants.status, 'destroyed'));

		let updated = 0;
		for (const t of allTenants) {
			const config = PLANS[t.plan as PlanId];
			if (!config) continue;
			await db()
				.update(tenants)
				.set({
					storageLimit: config.storageLimit,
					maxInstances: config.maxInstances,
					tokensPer4h: config.tokensPer4h,
					tokensPerMonth: config.tokensPerMonth,
					updatedAt: new Date(),
				})
				.where(eq(tenants.id, t.id));
			updated++;
		}

		return { success: true, updated };
	},

	resetLimits: async ({ request, locals }) => {
		if (!locals.user || !isAdmin(locals.user.email)) error(403, 'Forbidden');

		const form = await request.formData();
		const tenantId = form.get('tenantId') as string;
		if (!tenantId) return fail(400, { error: 'Missing tenantId' });

		const [existing] = await db()
			.select({ instanceId: rateLimits.instanceId })
			.from(rateLimits)
			.where(eq(rateLimits.instanceId, tenantId))
			.limit(1);

		if (!existing) return fail(404, { error: 'No rate limit record found' });

		await db()
			.update(rateLimits)
			.set({
				tokensLast4h: 0,
				tokensThisWeek: 0,
				tokensThisMonth: 0,
				lastReset4h: new Date(),
				lastResetWeekly: new Date(),
				lastResetMonthly: new Date(),
			})
			.where(eq(rateLimits.instanceId, tenantId));

		return { success: true, tenantId };
	},

	migrateSubscriptions: async ({ locals }) => {
		if (!locals.user || !isAdmin(locals.user.email)) error(403, 'Forbidden');

		const activeTenants = await db()
			.select({
				id: tenants.id,
				slug: tenants.slug,
				plan: tenants.plan,
				stripeSubscriptionId: tenants.stripeSubscriptionId,
			})
			.from(tenants)
			.where(ne(tenants.status, 'destroyed'));

		let migrated = 0;
		const errors: string[] = [];

		for (const t of activeTenants) {
			if (!t.stripeSubscriptionId) continue;
			try {
				const sub = await stripe().subscriptions.retrieve(t.stripeSubscriptionId);
				if (sub.status !== 'active' && sub.status !== 'trialing') continue;

				const item = sub.items.data[0];
				if (!item) continue;

				const newPriceId = priceIdForPlan(t.plan as PlanId);
				if (!newPriceId || item.price.id === newPriceId) continue;

				await stripe().subscriptions.update(t.stripeSubscriptionId, {
					items: [{ id: item.id, price: newPriceId }],
					proration_behavior: 'create_prorations',
				});
				migrated++;
			} catch (e) {
				const msg = e instanceof Error ? e.message : 'Unknown error';
				errors.push(`${t.slug}: ${msg}`);
			}
		}

		if (errors.length > 0) {
			return fail(500, { error: `Migrated ${migrated}, failed ${errors.length}: ${errors.join('; ')}` });
		}

		return { success: true, migrated };
	},

	notifyPriceChange: async ({ request, locals }) => {
		if (!locals.user || !isAdmin(locals.user.email)) error(403, 'Forbidden');

		const form = await request.formData();
		const message = (form.get('message') as string) || '';

		const activeTenants = await db()
			.select({
				slug: tenants.slug,
				plan: tenants.plan,
				userEmail: users.email,
				userName: users.name,
			})
			.from(tenants)
			.innerJoin(users, eq(tenants.userId, users.id))
			.where(ne(tenants.status, 'destroyed'));

		// Deduplicate by email
		const seen = new Set<string>();
		let sent = 0;
		const errors: string[] = [];

		for (const t of activeTenants) {
			if (seen.has(t.userEmail)) continue;
			seen.add(t.userEmail);

			const plan = PLANS[t.plan as PlanId];
			if (!plan) continue;

			try {
				await sendPriceChangeEmail(t.userEmail, t.userName ?? undefined, plan.name, message);
				sent++;
			} catch (e) {
				const msg = e instanceof Error ? e.message : 'Unknown error';
				errors.push(`${t.userEmail}: ${msg}`);
			}
		}

		if (errors.length > 0) {
			return fail(500, { error: `Sent ${sent}, failed ${errors.length}: ${errors.join('; ')}` });
		}

		return { success: true, sent };
	},

	provisionMachine: async ({ request, locals }) => {
		if (!locals.user || !isAdmin(locals.user.email)) error(403, 'Forbidden');

		const form = await request.formData();
		const tenantId = form.get('tenantId') as string;
		if (!tenantId) return fail(400, { error: 'Missing tenantId' });

		const [tenant] = await db()
			.select()
			.from(tenants)
			.innerJoin(users, eq(tenants.userId, users.id))
			.where(eq(tenants.id, tenantId))
			.limit(1);

		if (!tenant) return fail(404, { error: 'Tenant not found' });

		try {
			await provisionTenant({
				userId: tenant.tenants.userId,
				slug: tenant.tenants.slug,
				plan: tenant.tenants.plan as PlanId,
				stripeSubscriptionId: tenant.tenants.stripeSubscriptionId ?? undefined,
			});
		} catch (e) {
			const msg = e instanceof Error ? e.message : 'Unknown error';
			return fail(500, { error: `Provisioning failed: ${msg}` });
		}

		return { success: true, tenantId };
	},

	createMachine: async ({ request, locals }) => {
		if (!locals.user || !isAdmin(locals.user.email)) error(403, 'Forbidden');

		const form = await request.formData();
		const userId = form.get('userId') as string;
		const slug = form.get('slug') as string;
		const plan = (form.get('plan') as string) || 'companion';

		if (!userId || !slug) return fail(400, { error: 'userId and slug are required' });

		// Validate slug format
		if (!/^[a-z0-9][a-z0-9-]*[a-z0-9]$/.test(slug) || slug.length < 3 || slug.length > 30) {
			return fail(400, { error: 'Invalid slug: 3-30 chars, lowercase alphanumeric + hyphens' });
		}

		try {
			await provisionTenant({
				userId,
				slug,
				plan: plan as PlanId,
			});
		} catch (e) {
			const msg = e instanceof Error ? e.message : 'Unknown error';
			return fail(500, { error: `Provisioning failed: ${msg}` });
		}

		return { success: true };
	},

	patchEnv: async ({ locals }) => {
		if (!locals.user || !isAdmin(locals.user.email)) error(403, 'Forbidden');

		const allTenants = await db()
			.select()
			.from(tenants)
			.where(eq(tenants.status, 'running'));

		let patched = 0;
		const errors: string[] = [];

		for (const t of allTenants) {
			if (!t.flyAppId || !t.flyMachineId) continue;

			const isByok = !!(t.byokApiKey && t.byokProvider);

			// Build canonical env — BYOK machines get NO platform API keys
			const canonicalEnv: Record<string, string> = {
				BOLLY_HOME: '/data',
				RUST_LOG: 'info,rig=warn',
				BOLLY_CHANNEL: (t.imageChannel as string) ?? 'stable',
				BOLLY_AUTH_TOKEN: t.authToken ?? '',
				BOLLY_INSTANCE_ID: t.id,
				BOLLY_PUBLIC_URL: `https://${t.slug}.bollyai.dev`,
				DATABASE_URL: '', // explicitly blank out
				// Platform API keys — blank for BYOK, real for normal
				OPENROUTER_API_KEY: isByok ? '' : (env.OPENROUTER_API_KEY ?? ''),
				ANTHROPIC_API_KEY: isByok ? '' : (env.ANTHROPIC_API_KEY ?? ''),
				OPENAI_API_KEY: isByok ? '' : (env.OPENAI_API_KEY ?? ''),
				BRAVE_SEARCH_API_KEY: isByok ? '' : (env.BRAVE_SEARCH_API_KEY ?? ''),
				ELEVENLABS_API_KEY: isByok ? '' : (env.ELEVENLABS_API_KEY ?? ''),
				GOOGLE_CLIENT_ID: env.GOOGLE_CLIENT_ID ?? '',
				GOOGLE_CLIENT_SECRET: env.GOOGLE_CLIENT_SECRET ?? '',
				LANDING_URL: env.ORIGIN ?? '',
				BOLLY_RELEASE_TOKEN: env.BOLLY_RELEASE_TOKEN ?? '',
			};

			// BYOK: set only the user's own key for their chosen provider
			if (isByok) {
				canonicalEnv.BOLLY_BYOK = 'true';
				canonicalEnv.BOLLY_LLM_PROVIDER = t.byokProvider!;
				if (t.byokProvider === 'anthropic') canonicalEnv.ANTHROPIC_API_KEY = t.byokApiKey!;
				else if (t.byokProvider === 'openai') canonicalEnv.OPENAI_API_KEY = t.byokApiKey!;
				else if (t.byokProvider === 'openrouter') canonicalEnv.OPENROUTER_API_KEY = t.byokApiKey!;
				if (t.byokModel) canonicalEnv.BOLLY_LLM_MODEL = t.byokModel;
			}

			try {
				await fly.updateMachineEnv(t.flyAppId, t.flyMachineId, canonicalEnv);
				patched++;
			} catch (e) {
				const msg = e instanceof Error ? e.message : 'Unknown error';
				errors.push(`${t.slug}: ${msg}`);
			}
		}

		if (errors.length > 0) {
			return fail(500, { error: `Patched ${patched}, failed ${errors.length}: ${errors.join('; ')}` });
		}

		return { success: true, patched };
	},

	updateImage: async ({ request, locals }) => {
		if (!locals.user || !isAdmin(locals.user.email)) error(403, 'Forbidden');

		const form = await request.formData();
		const tenantId = form.get('tenantId') as string;
		if (!tenantId) return fail(400, { error: 'Missing tenantId' });

		const [tenant] = await db()
			.select({
				slug: tenants.slug,
				flyAppId: tenants.flyAppId,
				flyMachineId: tenants.flyMachineId,
				imageChannel: tenants.imageChannel,
				status: tenants.status,
				byokProvider: tenants.byokProvider,
			})
			.from(tenants)
			.where(eq(tenants.id, tenantId));

		if (!tenant) return fail(404, { error: 'Tenant not found' });
		if (!tenant.flyAppId || !tenant.flyMachineId) return fail(400, { error: 'No Fly machine' });

		try {
			// Sync env vars — skip platform API keys for BYOK machines
			const envPatch: Record<string, string> = {};
			if (!tenant.byokProvider) {
				if (env.ANTHROPIC_API_KEY) envPatch.ANTHROPIC_API_KEY = env.ANTHROPIC_API_KEY;
				if (env.OPENAI_API_KEY) envPatch.OPENAI_API_KEY = env.OPENAI_API_KEY;
				if (env.OPENROUTER_API_KEY) envPatch.OPENROUTER_API_KEY = env.OPENROUTER_API_KEY;
				if (env.BRAVE_SEARCH_API_KEY) envPatch.BRAVE_SEARCH_API_KEY = env.BRAVE_SEARCH_API_KEY;
				if (env.ELEVENLABS_API_KEY) envPatch.ELEVENLABS_API_KEY = env.ELEVENLABS_API_KEY;
			}
			if (env.GOOGLE_CLIENT_ID) envPatch.GOOGLE_CLIENT_ID = env.GOOGLE_CLIENT_ID;
			if (env.GOOGLE_CLIENT_SECRET) envPatch.GOOGLE_CLIENT_SECRET = env.GOOGLE_CLIENT_SECRET;

			if (Object.keys(envPatch).length > 0) {
				await fly.updateMachineEnv(tenant.flyAppId, tenant.flyMachineId, envPatch);
			}

			const image = fly.imageForChannel((tenant.imageChannel as fly.ImageChannel) ?? 'stable');
			await fly.updateMachineImage(tenant.flyAppId, tenant.flyMachineId, image);

			return { success: true, slug: tenant.slug };
		} catch (e) {
			const msg = e instanceof Error ? e.message : 'Unknown error';
			return fail(500, { error: `Failed to update ${tenant.slug}: ${msg}` });
		}
	},
};
