import { fail, redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types.js';
import { verifyEmail, createEmailVerificationToken } from '$lib/server/auth/index.js';
import { sendVerificationEmail } from '$lib/server/email/index.js';

export const load: PageServerLoad = async ({ url, locals }) => {
	const token = url.searchParams.get('token');

	if (token) {
		const ok = await verifyEmail(token);
		if (ok) {
			redirect(302, '/dashboard');
		}
		return { expired: true };
	}

	// No token — show "check your email" page
	if (locals.user?.emailVerified) {
		redirect(302, '/dashboard');
	}

	return { expired: false };
};

export const actions: Actions = {
	resend: async ({ locals }) => {
		if (!locals.user) redirect(302, '/login');
		if (locals.user.emailVerified) redirect(302, '/dashboard');

		try {
			const token = await createEmailVerificationToken(locals.user.id);
			await sendVerificationEmail(locals.user.email, token);
		} catch (e) {
			console.error('Failed to resend verification email:', e);
			return fail(500, { message: 'Failed to send email. Try again later.' });
		}

		return { resent: true };
	},
};
