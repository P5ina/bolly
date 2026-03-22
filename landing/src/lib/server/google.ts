import { GOOGLE_CLIENT_ID, GOOGLE_CLIENT_SECRET } from '$env/static/private';

const GOOGLE_AUTH_URL = 'https://accounts.google.com/o/oauth2/v2/auth';
const GOOGLE_TOKEN_URL = 'https://oauth2.googleapis.com/token';
const GOOGLE_USERINFO_URL = 'https://www.googleapis.com/oauth2/v2/userinfo';
const GOOGLE_REVOKE_URL = 'https://oauth2.googleapis.com/revoke';

const SCOPES = [
	'https://www.googleapis.com/auth/gmail.send',
	'https://www.googleapis.com/auth/gmail.readonly',
	'https://www.googleapis.com/auth/calendar.events',
	'https://www.googleapis.com/auth/drive',
	'https://www.googleapis.com/auth/userinfo.email',
].join(' ');

function clientId(): string {
	return GOOGLE_CLIENT_ID;
}

function clientSecret(): string {
	return GOOGLE_CLIENT_SECRET;
}

export function getGoogleAuthUrl(redirectUri: string, state: string): string {
	const params = new URLSearchParams({
		client_id: clientId(),
		redirect_uri: redirectUri,
		response_type: 'code',
		scope: SCOPES,
		access_type: 'offline',
		prompt: 'consent',
		state,
	});
	return `${GOOGLE_AUTH_URL}?${params.toString()}`;
}

export async function exchangeCode(
	code: string,
	redirectUri: string
): Promise<{
	access_token: string;
	refresh_token: string;
	expires_in: number;
	scope: string;
}> {
	const res = await fetch(GOOGLE_TOKEN_URL, {
		method: 'POST',
		headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
		body: new URLSearchParams({
			code,
			client_id: clientId(),
			client_secret: clientSecret(),
			redirect_uri: redirectUri,
			grant_type: 'authorization_code',
		}),
	});

	if (!res.ok) {
		const text = await res.text();
		throw new Error(`Google token exchange failed: ${res.status} ${text}`);
	}

	return res.json();
}

export async function refreshAccessToken(refreshToken: string): Promise<{
	access_token: string;
	expires_in: number;
}> {
	const res = await fetch(GOOGLE_TOKEN_URL, {
		method: 'POST',
		headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
		body: new URLSearchParams({
			refresh_token: refreshToken,
			client_id: clientId(),
			client_secret: clientSecret(),
			grant_type: 'refresh_token',
		}),
	});

	if (!res.ok) {
		const text = await res.text();
		throw new Error(`Google token refresh failed: ${res.status} ${text}`);
	}

	return res.json();
}

export async function getGoogleUserInfo(accessToken: string): Promise<{
	email: string;
	name?: string;
}> {
	const res = await fetch(GOOGLE_USERINFO_URL, {
		headers: { Authorization: `Bearer ${accessToken}` },
	});

	if (!res.ok) {
		throw new Error(`Google userinfo failed: ${res.status}`);
	}

	return res.json();
}

export async function revokeToken(token: string): Promise<void> {
	await fetch(`${GOOGLE_REVOKE_URL}?token=${token}`, { method: 'POST' });
}
