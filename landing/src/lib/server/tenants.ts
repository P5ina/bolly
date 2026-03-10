import { db } from './db/index.js';
import { tenants, type NewTenant, type Tenant } from './db/schema.js';
import { eq, and } from 'drizzle-orm';
import { generateId } from './auth/index.js';
import * as fly from './fly/index.js';
import { PLANS, type PlanId } from './stripe/index.js';

function appName(tenantId: string): string {
	return `bolly-${tenantId}`;
}

export async function provisionTenant(opts: {
	userId: string;
	slug: string;
	plan: PlanId;
	stripeSubscriptionId?: string;
}): Promise<Tenant> {
	const id = generateId();
	const authToken = generateId(32);
	const planConfig = PLANS[opts.plan];

	// Insert tenant record
	const [tenant] = await db()
		.insert(tenants)
		.values({
			id,
			userId: opts.userId,
			slug: opts.slug,
			plan: opts.plan,
			status: 'provisioning',
			authToken,
			stripeSubscriptionId: opts.stripeSubscriptionId,
			storageLimit: planConfig.storageLimit,
			maxInstances: planConfig.maxInstances,
		})
		.returning();

	try {
		// Create Fly app
		const app = await fly.createApp(appName(id));

		// Create volume
		const sizeGb = Math.max(1, Math.ceil(planConfig.storageLimit / 1024));
		const volume = await fly.createVolume(app.name, { sizeGb });

		// Create machine
		const machine = await fly.createMachine({
			appName: app.name,
			volumeId: volume.id,
			authToken,
		});

		// Update tenant with Fly details
		const [updated] = await db()
			.update(tenants)
			.set({
				flyAppId: app.name,
				flyMachineId: machine.id,
				flyVolumeId: volume.id,
				flyIp: machine.private_ip,
				status: 'running',
				updatedAt: new Date(),
			})
			.where(eq(tenants.id, id))
			.returning();

		return updated;
	} catch (err) {
		// Mark as error
		await db()
			.update(tenants)
			.set({ status: 'error', updatedAt: new Date() })
			.where(eq(tenants.id, id));

		// Try to clean up
		try {
			await fly.deleteApp(appName(id));
		} catch { /* ignore */ }

		throw err;
	}
}

export async function destroyTenant(tenantId: string): Promise<void> {
	const [tenant] = await db()
		.select()
		.from(tenants)
		.where(eq(tenants.id, tenantId))
		.limit(1);

	if (!tenant) return;

	if (tenant.flyAppId) {
		try {
			await fly.deleteApp(tenant.flyAppId);
		} catch { /* best effort */ }
	}

	await db()
		.update(tenants)
		.set({ status: 'destroyed', updatedAt: new Date() })
		.where(eq(tenants.id, tenantId));
}

export async function getTenantsByUser(userId: string): Promise<Tenant[]> {
	return db()
		.select()
		.from(tenants)
		.where(and(eq(tenants.userId, userId), eq(tenants.status, 'running')));
}

export async function getTenantBySlug(slug: string): Promise<Tenant | undefined> {
	const [tenant] = await db()
		.select()
		.from(tenants)
		.where(eq(tenants.slug, slug))
		.limit(1);
	return tenant;
}

export async function getTenantUrl(tenant: Tenant): Promise<string> {
	if (tenant.flyAppId) {
		return `https://${tenant.flyAppId}.fly.dev`;
	}
	throw new Error('Tenant has no Fly app');
}
