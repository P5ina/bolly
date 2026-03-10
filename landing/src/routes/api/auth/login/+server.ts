import { json, error } from '@sveltejs/kit';
import type { RequestHandler } from './$types.js';
import { db } from '$lib/server/db/index.js';
import { users } from '$lib/server/db/schema.js';
import { eq } from 'drizzle-orm';
import { verifyPassword, createSession, setSessionCookie } from '$lib/server/auth/index.js';

export const POST: RequestHandler = async ({ request, cookies }) => {
	const { email, password } = await request.json();

	if (!email || !password) {
		error(400, 'Email and password are required');
	}

	const [user] = await db()
		.select()
		.from(users)
		.where(eq(users.email, email.toLowerCase().trim()))
		.limit(1);

	if (!user) {
		error(401, 'Invalid email or password');
	}

	const valid = await verifyPassword(password, user.passwordHash);
	if (!valid) {
		error(401, 'Invalid email or password');
	}

	const sessionId = await createSession(user.id);
	setSessionCookie(cookies, sessionId);

	return json({ user: { id: user.id, email: user.email, name: user.name } });
};
