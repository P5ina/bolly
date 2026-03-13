import { error, fail, redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types.js';
import { db } from '$lib/server/db/index.js';
import { tenants, users, rateLimits } from '$lib/server/db/schema.js';
import { eq, ne } from 'drizzle-orm';
import { env } from '$env/dynamic/private';
import * as fly from '$lib/server/fly/index.js';
import { PLANS, type PlanId } from '$lib/server/stripe/index.js';

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
			messagesPerDay: tenants.messagesPerDay,
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
					}] as const;
				} catch {
					return [t.id, null] as const;
				}
			}),
	);
	const machineMap = Object.fromEntries(machineEnvs);

	return {
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
			})
			.from(tenants)
			.where(eq(tenants.status, 'running'));

		// Collect current API keys from landing env to push to all machines
		const envPatch: Record<string, string> = {};
		if (env.ANTHROPIC_API_KEY) envPatch.ANTHROPIC_API_KEY = env.ANTHROPIC_API_KEY;
		if (env.OPENAI_API_KEY) envPatch.OPENAI_API_KEY = env.OPENAI_API_KEY;
		if (env.OPENROUTER_API_KEY) envPatch.OPENROUTER_API_KEY = env.OPENROUTER_API_KEY;
		if (env.BRAVE_SEARCH_API_KEY) envPatch.BRAVE_SEARCH_API_KEY = env.BRAVE_SEARCH_API_KEY;
		if (env.GOOGLE_CLIENT_ID) envPatch.GOOGLE_CLIENT_ID = env.GOOGLE_CLIENT_ID;
		if (env.GOOGLE_CLIENT_SECRET) envPatch.GOOGLE_CLIENT_SECRET = env.GOOGLE_CLIENT_SECRET;

		let updated = 0;
		const errors: string[] = [];

		for (const t of allTenants) {
			if (!t.flyAppId || !t.flyMachineId) continue;
			try {
				// Sync env vars first (adds missing keys to old machines)
				if (Object.keys(envPatch).length > 0) {
					await fly.updateMachineEnv(t.flyAppId, t.flyMachineId, envPatch);
				}
				// Then update image
				const image = fly.imageForChannel((t.imageChannel as fly.ImageChannel) ?? 'stable');
				await fly.updateMachineImage(t.flyAppId, t.flyMachineId, image);
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
					messagesPerDay: config.messagesPerDay,
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
				messagesToday: 0,
				tokensThisMonth: 0,
				lastResetDaily: new Date(),
				lastResetMonthly: new Date(),
			})
			.where(eq(rateLimits.instanceId, tenantId));

		return { success: true, tenantId };
	},
};
