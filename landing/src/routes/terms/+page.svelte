<script lang="ts">
	import Footer from '$lib/components/Footer.svelte';
</script>

<svelte:head>
	<title>Terms of Service — Bolly</title>
</svelte:head>

<div class="min-h-dvh bg-bg">
	<header class="border-b border-border" style="background: oklch(0.05 0.015 280 / 70%); backdrop-filter: blur(24px);">
		<div class="mx-auto max-w-[900px] px-6 py-4">
			<a href="/" class="flex items-center gap-2.5 w-fit">
				<div class="w-7 h-7 rounded-md flex items-center justify-center font-display italic text-sm text-warm"
					style="background: var(--color-warm-glow); border: 1px solid var(--color-border-warm);"
				>b</div>
				<span class="font-display italic text-lg text-text">bolly</span>
			</a>
		</div>
	</header>

	<div class="mx-auto max-w-[700px] px-6 py-16">
		<h1 class="font-display italic text-3xl text-text mb-2">Terms of Service</h1>
		<p class="text-xs text-text-ghost mb-12">Last updated: March 13, 2026</p>

		<div class="prose">
			<h2>1. Agreement</h2>
			<p>
				By using Bolly ("the Service"), operated by Triangle Interactive ("we", "us"), you agree to these terms.
				If you don't agree, don't use the Service.
			</p>

			<h2>2. The Service</h2>
			<p>
				Bolly provides AI companion instances that run on dedicated servers. Each companion has persistent memory,
				tool access, and optional integrations (Google, web browsing, etc.). The companion uses third-party LLM
				providers to generate responses.
			</p>

			<h2>3. Accounts</h2>
			<p>You must provide a valid email address to create an account. You are responsible for keeping your account credentials secure. One person per account. You must be at least 13 years old to use the Service.</p>

			<h2>4. Subscriptions and billing</h2>
			<p>
				Paid plans are billed monthly via Stripe. You can cancel at any time — your instance remains active until the end of the billing period.
				Refunds are handled on a case-by-case basis. We reserve the right to change pricing with 30 days notice.
			</p>

			<h2>5. Your data</h2>
			<p>
				You own your data. Conversations, files, memories, and anything you create with your companion belong to you.
				We do not access your companion data except for technical support you explicitly request, or when required by law.
				See our <a href="/privacy">Privacy Policy</a> for details.
			</p>

			<h2>6. Google integrations</h2>
			<p>
				If you connect your Google account, you authorize your companion to access Gmail, Google Calendar, and
				Google Drive on your behalf. Your companion can send emails, read your inbox, create calendar events, and
				access drive files as directed by you or its autonomous behavior (heartbeat). You can revoke access at any
				time from the dashboard or from your
				<a href="https://myaccount.google.com/permissions" target="_blank">Google account settings</a>.
			</p>

			<h2>7. Acceptable use</h2>
			<p>You agree not to:</p>
			<ul>
				<li>Use the Service for illegal activity</li>
				<li>Send spam or bulk unsolicited messages via the email tools</li>
				<li>Attempt to access other users' instances or data</li>
				<li>Abuse rate limits or circumvent billing</li>
				<li>Use the Service to generate content that violates the LLM providers' usage policies</li>
				<li>Resell access to the Service without permission</li>
			</ul>
			<p>We may suspend or terminate accounts that violate these terms.</p>

			<h2>8. AI limitations</h2>
			<p>
				Your companion is an AI. It can make mistakes, hallucinate, or produce inaccurate information.
				It should not be relied upon for medical, legal, financial, or safety-critical decisions.
				Actions taken by your companion (sending emails, creating events, running commands) are your responsibility.
			</p>

			<h2>9. Availability</h2>
			<p>
				We aim for high availability but do not guarantee uninterrupted service. Maintenance, infrastructure
				issues, or third-party outages (Fly.io, LLM providers, Google APIs) may cause downtime. We are not
				liable for losses resulting from service interruptions.
			</p>

			<h2>10. Limitation of liability</h2>
			<p>
				The Service is provided "as is" without warranties of any kind. To the maximum extent permitted by law,
				we are not liable for any indirect, incidental, special, or consequential damages arising from your use
				of the Service. Our total liability is limited to the amount you paid us in the 12 months preceding the claim.
			</p>

			<h2>11. Termination</h2>
			<p>
				You can delete your account at any time. We can terminate your account for violation of these terms
				with notice. Upon termination, your companion instances are stopped and data is deleted after 30 days.
			</p>

			<h2>12. Changes</h2>
			<p>We may update these terms. Material changes will be communicated via email. Continued use after changes take effect constitutes acceptance.</p>

			<h2>13. Contact</h2>
			<p>Questions? Email <a href="mailto:support@bollyai.dev">support@bollyai.dev</a>.</p>
		</div>
	</div>

	<Footer />
</div>

<style>
	.prose {
		color: oklch(0.65 0.02 280);
		line-height: 1.75;
	}
	.prose h2 {
		color: oklch(0.88 0.02 280);
		font-size: 1.15rem;
		font-weight: 600;
		margin-top: 2.5rem;
		margin-bottom: 0.75rem;
	}
	.prose p {
		font-size: 0.875rem;
		margin-bottom: 1rem;
	}
	.prose ul {
		font-size: 0.875rem;
		padding-left: 1.5rem;
		margin-bottom: 1rem;
	}
	.prose li {
		margin-bottom: 0.35rem;
	}
	.prose a {
		color: oklch(0.78 0.12 75);
		text-decoration: underline;
		text-underline-offset: 2px;
	}
	.prose a:hover {
		color: oklch(0.85 0.12 75);
	}
</style>
