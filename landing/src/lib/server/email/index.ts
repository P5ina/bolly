import { Resend } from 'resend';
import { env } from '$env/dynamic/private';

let _resend: Resend;

function resend(): Resend {
	if (!_resend) {
		_resend = new Resend(env.RESEND_API_KEY);
	}
	return _resend;
}

const FROM = 'Bolly <noreply@mail.bollyai.dev>';

export async function sendPasswordResetEmail(to: string, token: string) {
	const resetUrl = `${env.ORIGIN ?? 'https://bollyai.dev'}/reset-password?token=${token}`;

	await resend().emails.send({
		from: FROM,
		to,
		subject: 'Reset your password',
		html: `
			<div style="font-family: system-ui, sans-serif; max-width: 480px; margin: 0 auto; padding: 40px 20px;">
				<h2 style="font-size: 20px; margin-bottom: 16px;">Reset your password</h2>
				<p style="color: #666; font-size: 14px; line-height: 1.6;">
					Someone requested a password reset for your Bolly account. Click the link below to choose a new password.
				</p>
				<a href="${resetUrl}" style="display: inline-block; margin: 24px 0; padding: 12px 24px; background: #1a1a1a; color: #fff; text-decoration: none; border-radius: 8px; font-size: 14px;">
					Reset password
				</a>
				<p style="color: #999; font-size: 12px;">
					This link expires in 1 hour. If you didn't request this, you can safely ignore this email.
				</p>
			</div>
		`,
	});
}
