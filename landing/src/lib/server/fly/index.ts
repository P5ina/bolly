import { FLY_API_TOKEN, GOOGLE_CLIENT_ID, GOOGLE_CLIENT_SECRET, ORIGIN } from '$env/static/private';

const FLY_API = 'https://api.machines.dev/v1';
const FLY_REGISTRY_APP = 'bolly';
const FLY_ORG = 'personal';
const FLY_REGION = 'iad';
const BOLLY_IMAGE = 'registry.fly.io/bolly:latest';

/** Shared keys pushed to ALL machines. */
export function sharedKeys(): Record<string, string> {
	return {
		GOOGLE_CLIENT_ID,
		GOOGLE_CLIENT_SECRET,
		LANDING_URL: ORIGIN,
	};
}

/** Build env for a machine. All instances are BYOK — no shared LLM keys. */
export function machineEnv(opts: {
	authToken: string;
	instanceId: string;
	publicUrl: string;
	channel?: string;
}): Record<string, string> {
	return {
		BOLLY_HOME: '/data',
		PORT: '8080',
		RUST_LOG: 'info,rig=warn',
		BOLLY_CHANNEL: opts.channel ?? 'stable',
		BOLLY_AUTH_TOKEN: opts.authToken,
		BOLLY_INSTANCE_ID: opts.instanceId,
		BOLLY_PUBLIC_URL: opts.publicUrl,
		DATABASE_URL: '',
		...sharedKeys(),
	};
}

/** The registry app that holds the Docker image */
export function registryApp(): string {
	return FLY_REGISTRY_APP;
}

export type ImageChannel = 'stable' | 'nightly';

export function imageForChannel(_channel: ImageChannel): string {
	// Single Docker image for all channels — nightly/stable only affects
	// which binary update-bolly.sh downloads inside the container.
	return `registry.fly.io/${registryApp()}:latest`;
}

function headers() {
	const auth = FLY_API_TOKEN.startsWith('FlyV1') ? FLY_API_TOKEN : `Bearer ${FLY_API_TOKEN}`;
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
			org_slug: FLY_ORG,
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
			region: opts.region ?? FLY_REGION,
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
			region: opts.region ?? FLY_REGION,
			config: {
				image: BOLLY_IMAGE || imageForChannel(opts.channel ?? 'stable'),
				env: machineEnv({
					authToken: opts.authToken,
					instanceId: opts.instanceId,
					publicUrl: opts.publicUrl,
					channel: opts.channel,
				}),
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

/** Atomic update: image + env in a single POST to avoid race conditions. */
export async function updateMachineImageAndEnv(
	appName: string,
	machineId: string,
	image: string,
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
				image,
				env: { ...machine.config.env, ...envPatch },
			},
		}),
	});
	if (!res.ok) throw new Error(`Fly updateMachine failed: ${res.status} ${await res.text()}`);
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

/** Replace the entire env (does NOT merge with existing). */
export async function replaceMachineEnv(
	appName: string,
	machineId: string,
	env: Record<string, string>,
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
				env,
			},
		}),
	});
	if (!res.ok) throw new Error(`Fly replaceMachineEnv failed: ${res.status} ${await res.text()}`);
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
