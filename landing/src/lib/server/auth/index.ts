import { db } from '../db/index.js';
import { emailVerificationTokens, passwordResetTokens, sessions, users, type User } from '../db/schema.js';
import { eq } from 'drizzle-orm';
import { sha256 } from '@oslojs/crypto/sha2';
import { encodeHexLowerCase } from '@oslojs/encoding';
import { dev } from '$app/environment';
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
		secure: !dev,
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

// ─── Email Verification ─────────────────────────────────────────────────────

const VERIFICATION_TOKEN_DURATION = 24 * 60 * 60 * 1000; // 24 hours

export async function createEmailVerificationToken(userId: string): Promise<string> {
	// Delete any existing tokens for this user
	await db().delete(emailVerificationTokens).where(eq(emailVerificationTokens.userId, userId));

	const token = generateId(40);
	const expiresAt = new Date(Date.now() + VERIFICATION_TOKEN_DURATION);

	await db().insert(emailVerificationTokens).values({ id: token, userId, expiresAt });
	return token;
}

export async function verifyEmail(token: string): Promise<boolean> {
	const result = await db()
		.select()
		.from(emailVerificationTokens)
		.where(eq(emailVerificationTokens.id, token))
		.limit(1);

	if (result.length === 0) return false;

	const row = result[0];
	if (row.expiresAt < new Date()) {
		await db().delete(emailVerificationTokens).where(eq(emailVerificationTokens.id, token));
		return false;
	}

	// Mark user as verified
	await db().update(users).set({ emailVerified: true }).where(eq(users.id, row.userId));
	// Clean up token
	await db().delete(emailVerificationTokens).where(eq(emailVerificationTokens.id, token));

	return true;
}

// ─── Password Reset ──────────────────────────────────────────────────────────

const RESET_TOKEN_DURATION = 60 * 60 * 1000; // 1 hour

export async function createPasswordResetToken(email: string): Promise<string | null> {
	const result = await db().select().from(users).where(eq(users.email, email)).limit(1);
	if (result.length === 0) return null;

	const user = result[0];
	const token = generateId(40);
	const expiresAt = new Date(Date.now() + RESET_TOKEN_DURATION);

	// Delete any existing tokens for this user
	await db().delete(passwordResetTokens).where(eq(passwordResetTokens.userId, user.id));

	await db().insert(passwordResetTokens).values({ id: token, userId: user.id, expiresAt });
	return token;
}

export async function validatePasswordResetToken(token: string): Promise<{ userId: string } | null> {
	const result = await db()
		.select()
		.from(passwordResetTokens)
		.where(eq(passwordResetTokens.id, token))
		.limit(1);

	if (result.length === 0) return null;

	const row = result[0];
	if (row.expiresAt < new Date()) {
		await db().delete(passwordResetTokens).where(eq(passwordResetTokens.id, token));
		return null;
	}

	return { userId: row.userId };
}

export async function resetPassword(token: string, newPassword: string): Promise<boolean> {
	const valid = await validatePasswordResetToken(token);
	if (!valid) return false;

	const hash = hashPassword(newPassword);
	await db().update(users).set({ passwordHash: hash }).where(eq(users.id, valid.userId));
	await db().delete(passwordResetTokens).where(eq(passwordResetTokens.id, token));

	// Invalidate all sessions for this user
	await db().delete(sessions).where(eq(sessions.userId, valid.userId));

	return true;
}
