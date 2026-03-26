import { Resend } from 'resend';
import { ORIGIN, RESEND_API_KEY } from '$env/static/private';

let _resend: Resend;

function resend(): Resend {
	if (!_resend) {
		_resend = new Resend(RESEND_API_KEY);
	}
	return _resend;
}

const FROM = 'Bolly <noreply@mail.bollyai.dev>';

export async function sendVerificationEmail(to: string, token: string) {
	const verifyUrl = `${ORIGIN}/verify-email?token=${token}`;

	await resend().emails.send({
		from: FROM,
		to,
		subject: 'Verify your email',
		html: `
			<div style="font-family: system-ui, sans-serif; max-width: 480px; margin: 0 auto; padding: 40px 20px;">
				<h2 style="font-size: 20px; margin-bottom: 16px;">Welcome to Bolly</h2>
				<p style="color: #666; font-size: 14px; line-height: 1.6;">
					Verify your email address to get started.
				</p>
				<a href="${verifyUrl}" style="display: inline-block; margin: 24px 0; padding: 12px 24px; background: #1a1a1a; color: #fff; text-decoration: none; border-radius: 8px; font-size: 14px;">
					Verify email
				</a>
				<p style="color: #999; font-size: 12px;">
					This link expires in 24 hours. If you didn't create an account, you can safely ignore this email.
				</p>
			</div>
		`,
	});
}

export async function sendPasswordResetEmail(to: string, token: string) {
	const resetUrl = `${ORIGIN}/reset-password?token=${token}`;

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

export async function sendPriceChangeEmail(to: string, name?: string, planName?: string, message?: string) {
	const greeting = name ? `Hi ${name},` : 'Hi,';
	const planLine = planName ? ` for the <strong>${planName}</strong> plan` : '';
	const customMessage = message
		? `<p style="color: #444; font-size: 14px; line-height: 1.6;">${message}</p>`
		: '';

	await resend().emails.send({
		from: FROM,
		to,
		subject: 'Upcoming pricing update — Bolly',
		html: `
			<div style="font-family: system-ui, sans-serif; max-width: 480px; margin: 0 auto; padding: 40px 20px;">
				<h2 style="font-size: 20px; margin-bottom: 16px;">Pricing update</h2>
				<p style="color: #444; font-size: 14px; line-height: 1.6;">
					${greeting}
				</p>
				<p style="color: #444; font-size: 14px; line-height: 1.6;">
					We're writing to let you know about an upcoming change to our pricing${planLine}.
					The new pricing will take effect on your next billing cycle.
				</p>
				${customMessage}
				<p style="color: #444; font-size: 14px; line-height: 1.6;">
					You can manage your subscription anytime from your
					<a href="${ORIGIN}/dashboard" style="color: #1a1a1a;">dashboard</a>.
				</p>
				<p style="color: #999; font-size: 12px; margin-top: 32px;">
					If you have any questions, just reply to this email.
				</p>
			</div>
		`,
	});
}
