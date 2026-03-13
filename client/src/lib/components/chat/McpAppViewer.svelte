<script lang="ts">
	import { AppBridge, PostMessageTransport } from "@modelcontextprotocol/ext-apps/app-bridge";

	let {
		html,
		toolName,
		toolInput,
		toolOutput = "",
	}: {
		html: string;
		toolName: string;
		toolInput: string;
		toolOutput?: string;
	} = $props();

	let iframe: HTMLIFrameElement | undefined = $state();
	let ready = $state(false);
	let fullscreen = $state(false);
	let collapsed = $state(false);
	let bridgeRef: AppBridge | undefined = $state();
	let resultSent = false;

	// Fullscreen overlay element — lives in document.body to escape stacking contexts
	let overlay: HTMLDivElement | undefined;

	function enterFullscreen() {
		if (fullscreen) return;
		fullscreen = true;
		bridgeRef?.sendHostContextChange({ displayMode: "fullscreen" });

		// Create overlay in body
		overlay = document.createElement("div");
		overlay.className = "mcp-fullscreen-overlay";
		overlay.innerHTML = `
			<div class="mcp-fs-header">
				<span class="mcp-fs-label">${toolName}</span>
				<button class="mcp-fs-close" title="Close fullscreen">✕ close</button>
			</div>
			<div class="mcp-fs-body"></div>
		`;
		overlay.querySelector(".mcp-fs-close")!.addEventListener("click", exitFullscreen);
		document.body.appendChild(overlay);

		// Move iframe into overlay
		if (iframe) {
			overlay.querySelector(".mcp-fs-body")!.appendChild(iframe);
		}
	}

	function exitFullscreen() {
		if (!fullscreen) return;
		fullscreen = false;
		bridgeRef?.sendHostContextChange({ displayMode: "inline" });

		// Move iframe back to inline container
		const inlineSlot = document.getElementById(`mcp-inline-${toolName}`);
		if (iframe && inlineSlot) {
			inlineSlot.appendChild(iframe);
		}

		// Remove overlay
		if (overlay) {
			overlay.remove();
			overlay = undefined;
		}
	}

	function sendResult(bridge: AppBridge, output: string) {
		if (resultSent || !output) return;
		resultSent = true;
		try {
			const parsed = JSON.parse(output);
			if (parsed && Array.isArray(parsed.content)) {
				bridge.sendToolResult(parsed);
			} else {
				bridge.sendToolResult({
					content: [{ type: "text", text: typeof parsed === "string" ? parsed : JSON.stringify(parsed) }],
				});
			}
		} catch {
			bridge.sendToolResult({
				content: [{ type: "text", text: output }],
			});
		}
	}

	$effect(() => {
		if (toolOutput && ready && bridgeRef && !resultSent) {
			sendResult(bridgeRef, toolOutput);
		}
	});

	$effect(() => {
		if (!iframe) return;

		const doc = iframe.contentDocument;
		if (!doc) return;
		doc.open();
		doc.write(html);
		doc.close();

		try {
			const style = doc.createElement("style");
			style.textContent = `:root { color-scheme: dark; }`;
			doc.head?.appendChild(style);
		} catch {};

		const iframeWindow = iframe.contentWindow!;

		const bridge = new AppBridge(
			null,
			{ name: "bolly", version: "1.0.0" },
			{ openLinks: {} },
			{
				hostContext: {
					theme: "dark",
					platform: "web",
					containerDimensions: { maxHeight: 600 },
					displayMode: "inline",
					availableDisplayModes: ["inline", "fullscreen"],
				},
			},
		);
		bridgeRef = bridge;
		resultSent = false;

		bridge.oninitialized = () => {
			ready = true;

			let args: Record<string, unknown> = {};
			try { args = JSON.parse(toolInput); } catch {}
			bridge.sendToolInput({ arguments: args });

			if (toolOutput) {
				sendResult(bridge, toolOutput);
			}
		};

		bridge.onsizechange = ({ width, height }) => {
			if (!iframe || fullscreen) return;
			if (height !== undefined) {
				iframe.style.height = `${Math.min(height, 600)}px`;
			}
			if (width !== undefined) {
				iframe.style.minWidth = `min(${width}px, 100%)`;
			}
		};

		bridge.onrequestdisplaymode = async (params) => {
			if (params.mode === "fullscreen") {
				enterFullscreen();
			} else {
				exitFullscreen();
			}
			return { mode: params.mode === "fullscreen" ? "fullscreen" : "inline" };
		};

		bridge.onopenlink = async (params) => {
			window.open(params.url, "_blank", "noopener,noreferrer");
			return {};
		};

		const transport = new PostMessageTransport(iframeWindow, iframeWindow);
		bridge.connect(transport);

		return () => {
			if (overlay) { overlay.remove(); overlay = undefined; }
			bridgeRef = undefined;
			bridge.close();
		};
	});
</script>

<div class="mcp-app" class:collapsed>
	<div class="mcp-app-header">
		<span class="mcp-app-label">{toolName}</span>
		<div class="mcp-app-controls">
			{#if !collapsed}
				<button class="mcp-app-btn" onclick={enterFullscreen} title="Fullscreen">⊞</button>
			{/if}
			<button class="mcp-app-btn" onclick={() => collapsed = !collapsed} title={collapsed ? "Expand" : "Collapse"}>
				{collapsed ? "+" : "−"}
			</button>
		</div>
	</div>
	<div id="mcp-inline-{toolName}" class="mcp-inline-slot" class:hidden={collapsed}>
		<iframe
			bind:this={iframe}
			sandbox="allow-scripts allow-same-origin"
			title={toolName}
			class="mcp-app-frame"
			class:loaded={ready}
		></iframe>
	</div>
</div>

<svelte:head>
	{@html `<style>
		.mcp-fullscreen-overlay {
			position: fixed;
			inset: 0;
			z-index: 99999;
			background: #111;
			display: flex;
			flex-direction: column;
		}
		.mcp-fs-header {
			display: flex;
			align-items: center;
			justify-content: space-between;
			padding: 0.5rem 1rem;
			background: #1a1a1a;
			border-bottom: 1px solid #333;
			flex-shrink: 0;
		}
		.mcp-fs-label {
			font-family: monospace;
			font-size: 0.7rem;
			color: #888;
			letter-spacing: 0.06em;
		}
		.mcp-fs-close {
			background: #333;
			border: 1px solid #555;
			border-radius: 6px;
			color: #eee;
			font-size: 0.8rem;
			cursor: pointer;
			padding: 0.3rem 0.75rem;
			font-family: monospace;
		}
		.mcp-fs-close:hover {
			background: #c44;
			border-color: #c44;
			color: #fff;
		}
		.mcp-fs-body {
			flex: 1;
			display: flex;
		}
		.mcp-fs-body iframe {
			flex: 1;
			width: 100%;
			height: 100% !important;
			border: none;
			border-radius: 0;
		}
	</style>`}
</svelte:head>

<style>
	.mcp-app {
		max-width: 100%;
		margin: 0.5rem 0;
		animation: app-enter 0.45s cubic-bezier(0.16, 1, 0.3, 1) both;
		position: relative;
	}

	@keyframes app-enter {
		from { opacity: 0; transform: translateY(6px); }
		to { opacity: 1; transform: translateY(0); }
	}

	.mcp-app-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 0.35rem;
	}

	.mcp-app-label {
		font-family: var(--font-mono);
		font-size: 0.6rem;
		color: oklch(0.78 0.12 75 / 40%);
		letter-spacing: 0.06em;
		text-transform: lowercase;
	}

	.mcp-app-controls {
		display: flex;
		gap: 0.25rem;
	}

	.mcp-app-btn {
		background: none;
		border: 1px solid oklch(0.78 0.12 75 / 15%);
		border-radius: 4px;
		color: oklch(0.6 0 0);
		font-size: 0.75rem;
		cursor: pointer;
		padding: 0 0.35rem;
		line-height: 1.4;
		font-family: var(--font-mono);
	}

	.mcp-app-btn:hover {
		color: oklch(0.85 0 0);
		border-color: oklch(0.78 0.12 75 / 30%);
	}

	.mcp-inline-slot {
		display: contents;
	}

	.hidden {
		display: none;
	}

	.mcp-app-frame {
		width: 100%;
		height: 480px;
		border: 1px solid oklch(0.78 0.12 75 / 10%);
		border-radius: 8px;
		background: oklch(0.10 0.01 280);
		opacity: 0;
		transition: opacity 0.3s ease;
		display: block;
	}

	.mcp-app-frame.loaded {
		opacity: 1;
	}

	.collapsed .mcp-app-header {
		margin-bottom: 0;
	}
</style>
