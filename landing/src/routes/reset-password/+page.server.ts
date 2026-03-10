import { fail } from '@sveltejs/kit';
import type { Actions } from './$types.js';
import { resetPassword, validatePasswordResetToken } from '$lib/server/auth/index.js';

export const actions: Actions = {
	default: async ({ request }) => {
		const data = await request.formData();
		const token = data.get('token')?.toString();
		const password = data.get('password')?.toString();
		const confirmPassword = data.get('confirmPassword')?.toString();

		if (!token || !password) {
			return fail(400, { message: 'Token and password are required' });
		}

		if (password.length < 8) {
			return fail(400, { message: 'Password must be at least 8 characters' });
		}

		if (password !== confirmPassword) {
			return fail(400, { message: 'Passwords do not match' });
		}

		const success = await resetPassword(token, password);

		if (!success) {
			return fail(400, { message: 'Invalid or expired reset link' });
		}

		return { success: true };
	},
};
