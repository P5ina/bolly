import { fail, redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types.js';
import { db } from '$lib/server/db/index.js';
import { users } from '$lib/server/db/schema.js';
import { eq } from 'drizzle-orm';
import { verifyPassword, createSession, setSessionCookie } from '$lib/server/auth/index.js';

export const load: PageServerLoad = async ({ locals, url }) => {
	const redirectTo = url.searchParams.get('redirect');
	if (locals.user) redirect(302, redirectTo || '/dashboard');
	return { redirect: redirectTo };
};

export const actions: Actions = {
	default: async ({ request, cookies, url }) => {
		const data = await request.formData();
		const email = data.get('email')?.toString()?.toLowerCase().trim();
		const password = data.get('password')?.toString();

		if (!email || !password) {
			return fail(400, { email, message: 'Email and password are required' });
		}

		const [user] = await db()
			.select()
			.from(users)
			.where(eq(users.email, email))
			.limit(1);

		if (!user || !verifyPassword(password, user.passwordHash)) {
			return fail(401, { email, message: 'Invalid email or password' });
		}

		const sessionId = await createSession(user.id);
		setSessionCookie(cookies, sessionId);

		if (!user.emailVerified) {
			redirect(302, '/verify-email');
		}

		const redirectTo = url.searchParams.get('redirect');
		redirect(302, redirectTo || '/dashboard');
	},
};
