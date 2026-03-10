import { env } from '$env/dynamic/private';

const FLY_API = 'https://api.machines.dev/v1';

/** The registry app that holds the Docker image */
export function registryApp(): string {
	return env.FLY_REGISTRY_APP ?? 'bolly';
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
				image: env.BOLLY_IMAGE ?? `registry.fly.io/${registryApp()}:latest`,
				env: {
					BOLLY_HOME: '/data',
					RUST_LOG: 'info',
					BOLLY_AUTH_TOKEN: opts.authToken,
				},
				guest: {
					cpus: opts.cpus ?? 1,
					memory_mb: opts.memoryMb ?? 256,
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

export async function getMachine(appName: string, machineId: string): Promise<{ id: string; state: string; private_ip: string }> {
	const res = await fetch(`${FLY_API}/apps/${appName}/machines/${machineId}`, {
		headers: headers(),
	});
	if (!res.ok) throw new Error(`Fly getMachine failed: ${res.status} ${await res.text()}`);
	return res.json();
}
