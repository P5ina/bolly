import { CLOUDFLARE_API_TOKEN, CLOUDFLARE_ZONE_ID } from '$env/static/private';

const BOLLY_DOMAIN = 'bollyai.dev';

const CF_API = 'https://api.cloudflare.com/client/v4';

function headers() {
	return {
		Authorization: `Bearer ${CLOUDFLARE_API_TOKEN}`,
		'Content-Type': 'application/json',
	};
}

function zoneId(): string {
	return CLOUDFLARE_ZONE_ID;
}

export function tenantHostname(slug: string): string {
	return `${slug}.${BOLLY_DOMAIN}`;
}

export async function createDnsRecord(opts: {
	slug: string;
	ipv4: string;
	ipv6: string;
}): Promise<{ v4Id: string; v6Id: string }> {
	const name = tenantHostname(opts.slug);

	// Create A record (IPv4)
	const v4Res = await fetch(`${CF_API}/zones/${zoneId()}/dns_records`, {
		method: 'POST',
		headers: headers(),
		body: JSON.stringify({
			type: 'A',
			name,
			content: opts.ipv4,
			proxied: false, // DNS-only so Fly handles TLS
			ttl: 1, // auto
		}),
	});
	if (!v4Res.ok) throw new Error(`Cloudflare A record failed: ${v4Res.status} ${await v4Res.text()}`);
	const v4Data = await v4Res.json();

	// Create AAAA record (IPv6)
	const v6Res = await fetch(`${CF_API}/zones/${zoneId()}/dns_records`, {
		method: 'POST',
		headers: headers(),
		body: JSON.stringify({
			type: 'AAAA',
			name,
			content: opts.ipv6,
			proxied: false,
			ttl: 1,
		}),
	});
	if (!v6Res.ok) throw new Error(`Cloudflare AAAA record failed: ${v6Res.status} ${await v6Res.text()}`);
	const v6Data = await v6Res.json();

	return {
		v4Id: v4Data.result.id,
		v6Id: v6Data.result.id,
	};
}

export async function deleteDnsRecords(slug: string): Promise<void> {
	const name = tenantHostname(slug);

	// List records for this hostname
	const res = await fetch(
		`${CF_API}/zones/${zoneId()}/dns_records?name=${encodeURIComponent(name)}`,
		{ headers: headers() }
	);
	if (!res.ok) return;

	const data = await res.json();
	const records = data.result ?? [];

	// Delete each record
	await Promise.all(
		records.map((r: { id: string }) =>
			fetch(`${CF_API}/zones/${zoneId()}/dns_records/${r.id}`, {
				method: 'DELETE',
				headers: headers(),
			})
		)
	);
}
