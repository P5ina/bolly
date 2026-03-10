import { json, error } from '@sveltejs/kit';
import type { RequestHandler } from './$types.js';
import { db } from '$lib/server/db/index.js';
import { users } from '$lib/server/db/schema.js';
import { generateId, hashPassword, createSession, setSessionCookie } from '$lib/server/auth/index.js';
import { createCustomer } from '$lib/server/stripe/index.js';

export const POST: RequestHandler = async ({ request, cookies }) => {
	const { email, password, name } = await request.json();

	if (!email || !password) {
		error(400, 'Email and password are required');
	}

	if (password.length < 8) {
		error(400, 'Password must be at least 8 characters');
	}

	const id = generateId();
	const passwordHash = await hashPassword(password);

	// Create Stripe customer
	let stripeCustomerId: string | undefined;
	try {
		stripeCustomerId = await createCustomer(email, name);
	} catch {
		// Stripe not configured yet, that's ok
	}

	try {
		await db().insert(users).values({
			id,
			email: email.toLowerCase().trim(),
			passwordHash,
			name: name || null,
			stripeCustomerId: stripeCustomerId || null,
		});
	} catch (err: any) {
		if (err.message?.includes('unique') || err.code === '23505') {
			error(409, 'Email already registered');
		}
		throw err;
	}

	const sessionId = await createSession(id);
	setSessionCookie(cookies, sessionId);

	return json({ user: { id, email, name } });
};
