import { env } from '$env/dynamic/private';

const FLY_API = 'https://api.machines.dev/v1';

/** The registry app that holds the Docker image */
export function registryApp(): string {
	return env.FLY_REGISTRY_APP ?? 'bolly';
}

export type ImageChannel = 'stable' | 'nightly';

export function imageForChannel(channel: ImageChannel): string {
	const registry = `registry.fly.io/${registryApp()}`;
	return channel === 'nightly' ? `${registry}:nightly` : `${registry}:latest`;
}

function headers() {
	const raw = env.FLY_API_TOKEN ?? '';
	const auth = raw.startsWith('FlyV1') ? raw : `Bearer ${raw}`;
	return {
		Authorization: auth,
		'Content-Type': 'application/json',
	};
}

// ─── Apps ────────────────────────────────────────────────────────────────────

export async function createApp(name: string): Promise<{ id: string; name: string }> {
	const res = await fetch(`${FLY_API}/apps`, {
		method: 'POST',
		headers: headers(),
		body: JSON.stringify({
			app_name: name,
			org_slug: env.FLY_ORG ?? 'personal',
		}),
	});
	if (!res.ok) throw new Error(`Fly createApp failed: ${res.status} ${await res.text()}`);
	const data = await res.json();
	return { id: data.id, name };
}

export async function deleteApp(name: string): Promise<void> {
	const res = await fetch(`${FLY_API}/apps/${name}`, {
		method: 'DELETE',
		headers: headers(),
	});
	if (!res.ok && res.status !== 404) {
		throw new Error(`Fly deleteApp failed: ${res.status} ${await res.text()}`);
	}
}

// ─── IPs ─────────────────────────────────────────────────────────────────────

export async function allocateIpv4(appName: string): Promise<string> {
	const res = await fetch(`https://api.fly.io/graphql`, {
		method: 'POST',
		headers: headers(),
		body: JSON.stringify({
			query: `mutation($input: AllocateIPAddressInput!) { allocateIpAddress(input: $input) { ipAddress { id address type } } }`,
			variables: {
				input: {
					appId: appName,
					type: 'v4',
				},
			},
		}),
	});
	if (!res.ok) throw new Error(`Fly allocateIp failed: ${res.status} ${await res.text()}`);
	const data = await res.json();
	if (data.errors?.length) throw new Error(`Fly allocateIp: ${data.errors[0].message}`);
	const ip = data.data?.allocateIpAddress?.ipAddress?.address;
	if (!ip) throw new Error(`Fly allocateIp: unexpected response: ${JSON.stringify(data)}`);
	return ip;
}

export async function allocateIpv6(appName: string): Promise<string> {
	const res = await fetch(`https://api.fly.io/graphql`, {
		method: 'POST',
		headers: headers(),
		body: JSON.stringify({
			query: `mutation($input: AllocateIPAddressInput!) { allocateIpAddress(input: $input) { ipAddress { id address type } } }`,
			variables: {
				input: {
					appId: appName,
					type: 'v6',
				},
			},
		}),
	});
	if (!res.ok) throw new Error(`Fly allocateIpv6 failed: ${res.status} ${await res.text()}`);
	const data = await res.json();
	if (data.errors?.length) throw new Error(`Fly allocateIpv6: ${data.errors[0].message}`);
	const ip = data.data?.allocateIpAddress?.ipAddress?.address;
	if (!ip) throw new Error(`Fly allocateIpv6: unexpected response: ${JSON.stringify(data)}`);
	return ip;
}

// ─── Certificates ────────────────────────────────────────────────────────────

export async function addCertificate(appName: string, hostname: string): Promise<void> {
	const res = await fetch(`${FLY_API}/apps/${appName}/certificates/acme`, {
		method: 'POST',
		headers: headers(),
		body: JSON.stringify({ hostname }),
	});
	if (!res.ok) throw new Error(`Fly addCertificate failed: ${res.status} ${await res.text()}`);
}

// ─── Volumes ─────────────────────────────────────────────────────────────────

export async function createVolume(
	appName: string,
	opts: { name?: string; sizeGb?: number; region?: string } = {}
): Promise<{ id: string; name: string }> {
	const res = await fetch(`${FLY_API}/apps/${appName}/volumes`, {
		method: 'POST',
		headers: headers(),
		body: JSON.stringify({
			name: opts.name ?? 'data',
			size_gb: opts.sizeGb ?? 1,
			region: opts.region ?? env.FLY_REGION ?? 'iad',
		}),
	});
	if (!res.ok) throw new Error(`Fly createVolume failed: ${res.status} ${await res.text()}`);
	return res.json();
}

// ─── Machines ────────────────────────────────────────────────────────────────

export interface CreateMachineOpts {
	appName: string;
	volumeId: string;
	authToken: string;
	instanceId: string;
	publicUrl: string;
	channel?: ImageChannel;
	region?: string;
	cpus?: number;
	memoryMb?: number;
}

export async function createMachine(opts: CreateMachineOpts): Promise<{
	id: string;
	instance_id: string;
	private_ip: string;
}> {
	const res = await fetch(`${FLY_API}/apps/${opts.appName}/machines`, {
		method: 'POST',
		headers: headers(),
		body: JSON.stringify({
			region: opts.region ?? env.FLY_REGION ?? 'iad',
			config: {
				image: env.BOLLY_IMAGE ?? imageForChannel(opts.channel ?? 'stable'),
				env: {
					BOLLY_HOME: '/data',
					RUST_LOG: 'info,rig=warn',
					BOLLY_AUTH_TOKEN: opts.authToken,
					BOLLY_INSTANCE_ID: opts.instanceId,
					BOLLY_PUBLIC_URL: opts.publicUrl,
					DATABASE_URL: env.DATABASE_URL ?? '',
					OPENROUTER_API_KEY: env.OPENROUTER_API_KEY ?? '',
					ANTHROPIC_API_KEY: env.ANTHROPIC_API_KEY ?? '',
					OPENAI_API_KEY: env.OPENAI_API_KEY ?? '',
					BRAVE_SEARCH_API_KEY: env.BRAVE_SEARCH_API_KEY ?? '',
					GOOGLE_CLIENT_ID: env.GOOGLE_CLIENT_ID ?? '',
					GOOGLE_CLIENT_SECRET: env.GOOGLE_CLIENT_SECRET ?? '',
					LANDING_URL: env.ORIGIN ?? '',
				},
				guest: {
					cpus: opts.cpus ?? 1,
					memory_mb: opts.memoryMb ?? 2048,
					cpu_kind: 'shared',
				},
				mounts: [
					{
						volume: opts.volumeId,
						path: '/data',
					},
				],
				services: [
					{
						ports: [
							{ port: 443, handlers: ['tls', 'http'] },
							{ port: 80, handlers: ['http'] },
						],
						protocol: 'tcp',
						internal_port: 8080,
					},
				],
			},
		}),
	});
	if (!res.ok) throw new Error(`Fly createMachine failed: ${res.status} ${await res.text()}`);
	return res.json();
}

export async function destroyMachine(appName: string, machineId: string): Promise<void> {
	const res = await fetch(`${FLY_API}/apps/${appName}/machines/${machineId}?force=true`, {
		method: 'DELETE',
		headers: headers(),
	});
	if (!res.ok && res.status !== 404) {
		throw new Error(`Fly destroyMachine failed: ${res.status} ${await res.text()}`);
	}
}

export async function updateMachineImage(appName: string, machineId: string, image: string): Promise<void> {
	// Get current config
	const getRes = await fetch(`${FLY_API}/apps/${appName}/machines/${machineId}`, {
		headers: headers(),
	});
	if (!getRes.ok) throw new Error(`Fly getMachine failed: ${getRes.status} ${await getRes.text()}`);
	const machine = await getRes.json();

	// Update with new image
	const res = await fetch(`${FLY_API}/apps/${appName}/machines/${machineId}`, {
		method: 'POST',
		headers: headers(),
		body: JSON.stringify({
			config: { ...machine.config, image },
		}),
	});
	if (!res.ok) throw new Error(`Fly updateMachine failed: ${res.status} ${await res.text()}`);
}

export async function updateMachineEnv(
	appName: string,
	machineId: string,
	envPatch: Record<string, string>,
): Promise<void> {
	const getRes = await fetch(`${FLY_API}/apps/${appName}/machines/${machineId}`, {
		headers: headers(),
	});
	if (!getRes.ok) throw new Error(`Fly getMachine failed: ${getRes.status} ${await getRes.text()}`);
	const machine = await getRes.json();

	const res = await fetch(`${FLY_API}/apps/${appName}/machines/${machineId}`, {
		method: 'POST',
		headers: headers(),
		body: JSON.stringify({
			config: {
				...machine.config,
				env: { ...machine.config.env, ...envPatch },
			},
		}),
	});
	if (!res.ok) throw new Error(`Fly updateMachineEnv failed: ${res.status} ${await res.text()}`);
}

export async function getMachine(appName: string, machineId: string): Promise<{ id: string; state: string; private_ip: string; config?: { env?: Record<string, string> } }> {
	const res = await fetch(`${FLY_API}/apps/${appName}/machines/${machineId}`, {
		headers: headers(),
	});
	if (!res.ok) throw new Error(`Fly getMachine failed: ${res.status} ${await res.text()}`);
	return res.json();
}

export async function stopMachine(appName: string, machineId: string): Promise<void> {
	const res = await fetch(`${FLY_API}/apps/${appName}/machines/${machineId}/stop`, {
		method: 'POST',
		headers: headers(),
	});
	if (!res.ok) throw new Error(`Fly stopMachine failed: ${res.status} ${await res.text()}`);
}

export async function startMachine(appName: string, machineId: string): Promise<void> {
	const res = await fetch(`${FLY_API}/apps/${appName}/machines/${machineId}/start`, {
		method: 'POST',
		headers: headers(),
	});
	if (!res.ok) throw new Error(`Fly startMachine failed: ${res.status} ${await res.text()}`);
}
