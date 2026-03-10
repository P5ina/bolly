import { fail } from '@sveltejs/kit';
import type { Actions } from './$types.js';
import { createPasswordResetToken } from '$lib/server/auth/index.js';
import { sendPasswordResetEmail } from '$lib/server/email/index.js';

export const actions: Actions = {
	default: async ({ request }) => {
		const data = await request.formData();
		const email = data.get('email')?.toString()?.toLowerCase().trim();

		if (!email) {
			return fail(400, { message: 'Email is required' });
		}

		const token = await createPasswordResetToken(email);

		if (token) {
			try {
				await sendPasswordResetEmail(email, token);
			} catch (e) {
				console.error('Failed to send reset email:', e);
			}
		}

		return { sent: true };
	},
};
