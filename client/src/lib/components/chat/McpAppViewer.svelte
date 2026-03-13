<script lang="ts">
	let {
		html,
		toolName,
		toolInput,
		toolOutput,
	}: {
		html: string;
		toolName: string;
		toolInput: string;
		toolOutput: string;
	} = $props();

	let iframe: HTMLIFrameElement | undefined = $state();

	function handleMessage(event: MessageEvent) {
		if (!iframe || event.source !== iframe.contentWindow) return;

		const msg = event.data;
		if (msg?.type === "ui/ready") {
			// App is ready — send initialize, then tool input + result
			iframe.contentWindow?.postMessage(
				{
					type: "ui/initialize",
					toolName,
				},
				"*",
			);
			iframe.contentWindow?.postMessage(
				{
					type: "ui/notifications/tool-input",
					input: JSON.parse(toolInput),
				},
				"*",
			);
			iframe.contentWindow?.postMessage(
				{
					type: "ui/notifications/tool-result",
					result: toolOutput,
				},
				"*",
			);
		}
	}

	$effect(() => {
		window.addEventListener("message", handleMessage);
		return () => window.removeEventListener("message", handleMessage);
	});

	$effect(() => {
		if (!iframe) return;
		const doc = iframe.contentDocument;
		if (!doc) return;
		doc.open();
		doc.write(html);
		doc.close();
	});
</script>

<div class="mcp-app">
	<div class="mcp-app-label">{toolName}</div>
	<iframe
		bind:this={iframe}
		sandbox="allow-scripts allow-same-origin"
		title={toolName}
		class="mcp-app-frame"
	></iframe>
</div>

<style>
	.mcp-app {
		max-width: 100%;
		margin: 0.5rem 0;
		animation: app-enter 0.45s cubic-bezier(0.16, 1, 0.3, 1) both;
	}

	@keyframes app-enter {
		from {
			opacity: 0;
			transform: translateY(6px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
		}
	}

	.mcp-app-label {
		font-family: var(--font-mono);
		font-size: 0.6rem;
		color: oklch(0.78 0.12 75 / 40%);
		letter-spacing: 0.06em;
		text-transform: lowercase;
		margin-bottom: 0.35rem;
	}

	.mcp-app-frame {
		width: 100%;
		height: 400px;
		border: 1px solid oklch(0.78 0.12 75 / 10%);
		border-radius: 8px;
		background: oklch(0.10 0.01 280);
	}
</style>
