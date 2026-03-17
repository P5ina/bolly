import { db } from './db/index.js';
import { tenants, type Tenant } from './db/schema.js';
import { eq, and, ne } from 'drizzle-orm';
import { generateId } from './auth/index.js';
import * as fly from './fly/index.js';
import * as cf from './cloudflare/index.js';
import { PLANS, type PlanId } from './stripe/index.js';

function appName(slug: string): string {
	return `bolly-${slug}`;
}

export async function provisionTenant(opts: {
	userId: string;
	slug: string;
	plan: PlanId;
	stripeSubscriptionId?: string;
}): Promise<Tenant> {
	const planConfig = PLANS[opts.plan];
	const flyApp = appName(opts.slug);

	// Clean up any previous failed attempt with the same slug
	const [existing] = await db()
		.select()
		.from(tenants)
		.where(and(eq(tenants.slug, opts.slug), eq(tenants.userId, opts.userId)))
		.limit(1);

	if (existing) {
		if (existing.status === 'running') return existing;
		if (existing.flyAppId) {
			try { await fly.deleteApp(existing.flyAppId); } catch { /* ignore */ }
		}
		try { await cf.deleteDnsRecords(opts.slug); } catch { /* ignore */ }
		await db().delete(tenants).where(eq(tenants.id, existing.id));
	}

	const id = generateId();
	const authToken = generateId(32);
	const shareToken = generateId(32);

	const [tenant] = await db()
		.insert(tenants)
		.values({
			id,
			userId: opts.userId,
			slug: opts.slug,
			plan: opts.plan,
			status: 'provisioning',
			authToken,
			shareToken,
			stripeSubscriptionId: opts.stripeSubscriptionId,
			storageLimit: planConfig.storageLimit,
			maxInstances: planConfig.maxInstances,
			tokensPer4h: planConfig.tokensPer4h,
			tokensPerMonth: planConfig.tokensPerMonth,
		})
		.returning();

	try {
		// 1. Create dedicated Fly app
		const app = await fly.createApp(flyApp);

		// 2. Allocate IPs
		const [ipv4, ipv6] = await Promise.all([
			fly.allocateIpv4(app.name),
			fly.allocateIpv6(app.name),
		]);

		// 3. Create DNS records on Cloudflare + TLS certificate on Fly (in parallel)
		const hostname = cf.tenantHostname(opts.slug);
		await Promise.all([
			cf.createDnsRecord({ slug: opts.slug, ipv4, ipv6 }),
			fly.addCertificate(app.name, hostname),
		]);

		// 4. Create volume
		const sizeGb = Math.max(1, Math.ceil(planConfig.storageLimit / 1024));
		const volume = await fly.createVolume(app.name, { sizeGb });

		// 5. Create machine
		const machine = await fly.createMachine({
			appName: app.name,
			volumeId: volume.id,
			authToken,
			instanceId: id,
			publicUrl: `https://${hostname}`,
			channel: (tenant.imageChannel as fly.ImageChannel) ?? 'stable',
			cpus: planConfig.cpus,
			memoryMb: planConfig.memoryMb,
		});

		// 6. Update tenant
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
		const message = err instanceof Error ? err.message : 'Unknown provisioning error';

		await db()
			.update(tenants)
			.set({ status: 'error', errorMessage: message, updatedAt: new Date() })
			.where(eq(tenants.id, id));

		// Clean up
		try { await fly.deleteApp(flyApp); } catch { /* ignore */ }
		try { await cf.deleteDnsRecords(opts.slug); } catch { /* ignore */ }

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
		try { await fly.deleteApp(tenant.flyAppId); } catch { /* best effort */ }
	}
	try { await cf.deleteDnsRecords(tenant.slug); } catch { /* best effort */ }

	await db()
		.update(tenants)
		.set({ status: 'destroyed', updatedAt: new Date() })
		.where(eq(tenants.id, tenantId));
}

export async function getTenantsByUser(userId: string): Promise<Tenant[]> {
	return db()
		.select()
		.from(tenants)
		.where(and(eq(tenants.userId, userId), ne(tenants.status, 'destroyed')));
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
	return `https://${cf.tenantHostname(tenant.slug)}`;
}

export async function switchTenantChannel(
	tenantId: string,
	channel: fly.ImageChannel
): Promise<void> {
	const [tenant] = await db()
		.select()
		.from(tenants)
		.where(eq(tenants.id, tenantId))
		.limit(1);

	if (!tenant || !tenant.flyAppId || !tenant.flyMachineId) {
		throw new Error('Tenant not found or not running');
	}

	// Update the machine image on Fly
	await fly.updateMachineImage(tenant.flyAppId, tenant.flyMachineId, fly.imageForChannel(channel));

	// Update DB
	await db()
		.update(tenants)
		.set({ imageChannel: channel, updatedAt: new Date() })
		.where(eq(tenants.id, tenantId));
}
