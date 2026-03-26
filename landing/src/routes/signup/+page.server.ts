import { fail, redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types.js';
import { db } from '$lib/server/db/index.js';
import { users } from '$lib/server/db/schema.js';
import { generateId, hashPassword, createSession, setSessionCookie, createEmailVerificationToken } from '$lib/server/auth/index.js';
import { createCustomer } from '$lib/server/stripe/index.js';
import { sendVerificationEmail } from '$lib/server/email/index.js';

export const load: PageServerLoad = async ({ locals }) => {
	if (locals.user) redirect(302, '/dashboard');
};

export const actions: Actions = {
	default: async ({ request, cookies }) => {
		const data = await request.formData();
		const email = data.get('email')?.toString()?.toLowerCase().trim();
		const password = data.get('password')?.toString();
		const name = data.get('name')?.toString() || null;

		if (!email || !password) {
			return fail(400, { email, name, message: 'Email and password are required' });
		}

		if (password.length < 8) {
			return fail(400, { email, name, message: 'Password must be at least 8 characters' });
		}

		const id = generateId();
		const passwordHash = hashPassword(password);

		let stripeCustomerId: string | undefined;
		try {
			stripeCustomerId = await createCustomer(email, name ?? undefined);
		} catch {
			// Stripe not configured yet
		}

		try {
			await db().insert(users).values({
				id,
				email,
				passwordHash,
				name,
				stripeCustomerId: stripeCustomerId || null,
			});
		} catch (err: any) {
			if (err.message?.includes('unique') || err.code === '23505') {
				return fail(409, { email, name, message: 'Email already registered' });
			}
			throw err;
		}

		// Send verification email
		try {
			const token = await createEmailVerificationToken(id);
			await sendVerificationEmail(email, token);
		} catch (e) {
			console.error('Failed to send verification email:', e);
		}

		// Create session (user can browse but dashboard is gated)
		const sessionId = await createSession(id);
		setSessionCookie(cookies, sessionId);

		redirect(302, '/verify-email');
	},
};
