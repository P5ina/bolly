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

	function toggleFullscreen() {
		fullscreen = !fullscreen;
		bridgeRef?.sendHostContextChange({ displayMode: fullscreen ? "fullscreen" : "inline" });
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

	// When toolOutput arrives later (via WebSocket), send it to the bridge
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

			// Send result immediately if already available (e.g. page reload)
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
			const mode = params.mode === "fullscreen" ? "fullscreen" : "inline";
			fullscreen = mode === "fullscreen";
			bridge.sendHostContextChange({ displayMode: mode });
			return { mode };
		};

		bridge.onopenlink = async (params) => {
			window.open(params.url, "_blank", "noopener,noreferrer");
			return {};
		};

		const transport = new PostMessageTransport(iframeWindow, iframeWindow);
		bridge.connect(transport);

		return () => {
			bridgeRef = undefined;
			bridge.close();
		};
	});
</script>

<div class="mcp-app" class:fullscreen class:collapsed>
	<div class="mcp-app-header">
		<span class="mcp-app-label">{toolName}</span>
		<div class="mcp-app-controls">
			{#if !collapsed}
				<button class="mcp-app-btn" onclick={toggleFullscreen} title={fullscreen ? "Exit fullscreen" : "Fullscreen"}>
					{fullscreen ? "⊡" : "⊞"}
				</button>
			{/if}
			<button class="mcp-app-btn" onclick={() => { if (fullscreen) { fullscreen = false; bridgeRef?.sendHostContextChange({ displayMode: "inline" }); } collapsed = !collapsed; }} title={collapsed ? "Expand" : "Collapse"}>
				{collapsed ? "+" : "−"}
			</button>
		</div>
	</div>
	{#if !collapsed}
		<iframe
			bind:this={iframe}
			sandbox="allow-scripts allow-same-origin"
			title={toolName}
			class="mcp-app-frame"
			class:loaded={ready}
		></iframe>
	{/if}
</div>

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

	.fullscreen {
		position: fixed;
		inset: 0;
		z-index: 9999;
		margin: 0;
		background: oklch(0.08 0.01 280);
		display: flex;
		flex-direction: column;
		animation: none;
		padding: 0;
	}

	.fullscreen .mcp-app-header {
		padding: 0.4rem 0.75rem;
		margin: 0;
		border-bottom: 1px solid oklch(0.78 0.12 75 / 10%);
		flex-shrink: 0;
	}

	.fullscreen .mcp-app-frame {
		flex: 1;
		height: auto;
		border: none;
		border-radius: 0;
	}
</style>
