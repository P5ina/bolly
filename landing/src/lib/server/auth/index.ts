import { db } from '../db/index.js';
import { sessions, users, type User } from '../db/schema.js';
import { eq } from 'drizzle-orm';
import { sha256 } from '@oslojs/crypto/sha2';
import { encodeHexLowerCase } from '@oslojs/encoding';
import type { Cookies } from '@sveltejs/kit';

const SESSION_COOKIE = 'bolly_session';
const SESSION_DURATION = 30 * 24 * 60 * 60 * 1000; // 30 days

export function generateId(length = 21): string {
	const chars = 'abcdefghijklmnopqrstuvwxyz0123456789';
	const bytes = crypto.getRandomValues(new Uint8Array(length));
	return Array.from(bytes, (b) => chars[b % chars.length]).join('');
}

export function hashPassword(password: string): string {
	const encoded = new TextEncoder().encode(password);
	const hash = sha256(encoded);
	return encodeHexLowerCase(hash);
}

export function verifyPassword(password: string, hash: string): boolean {
	return hashPassword(password) === hash;
}

export async function createSession(userId: string): Promise<string> {
	const id = generateId(40);
	const expiresAt = new Date(Date.now() + SESSION_DURATION);

	await db().insert(sessions).values({ id, userId, expiresAt });
	return id;
}

export async function validateSession(sessionId: string): Promise<{ user: User; sessionId: string } | null> {
	const result = await db()
		.select({ session: sessions, user: users })
		.from(sessions)
		.innerJoin(users, eq(sessions.userId, users.id))
		.where(eq(sessions.id, sessionId))
		.limit(1);

	if (result.length === 0) return null;

	const { session, user } = result[0];

	if (session.expiresAt < new Date()) {
		await db().delete(sessions).where(eq(sessions.id, sessionId));
		return null;
	}

	return { user, sessionId: session.id };
}

export async function invalidateSession(sessionId: string): Promise<void> {
	await db().delete(sessions).where(eq(sessions.id, sessionId));
}

export function setSessionCookie(cookies: Cookies, sessionId: string) {
	cookies.set(SESSION_COOKIE, sessionId, {
		path: '/',
		httpOnly: true,
		secure: true,
		sameSite: 'lax',
		maxAge: SESSION_DURATION / 1000,
	});
}

export function deleteSessionCookie(cookies: Cookies) {
	cookies.delete(SESSION_COOKIE, { path: '/' });
}

export function getSessionId(cookies: Cookies): string | undefined {
	return cookies.get(SESSION_COOKIE);
}
