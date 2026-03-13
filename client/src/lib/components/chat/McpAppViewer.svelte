<script lang="ts">
	import { AppBridge, PostMessageTransport } from "@modelcontextprotocol/ext-apps/app-bridge";

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

	let container: HTMLDivElement | undefined = $state();
	let iframe: HTMLIFrameElement | undefined = $state();
	let ready = $state(false);
	let fullscreen = $state(false);

	$effect(() => {
		if (!iframe) return;

		// Write HTML into iframe first
		const doc = iframe.contentDocument;
		if (!doc) return;
		doc.open();
		doc.write(html);
		doc.close();

		// Capture contentWindow AFTER doc.write — reference may change
		const iframeWindow = iframe.contentWindow!;

		// Create AppBridge with null client (MCP connection is on the Rust server)
		const bridge = new AppBridge(
			null,
			{ name: "bolly", version: "1.0.0" },
			{
				openLinks: {},
			},
			{
				hostContext: {
					theme: "dark",
					platform: "web",
					containerDimensions: { maxHeight: 6000 },
					displayMode: "inline",
					availableDisplayModes: ["inline", "fullscreen"],
				},
			},
		);

		// When view finishes initialization handshake, send tool data
		bridge.oninitialized = () => {
			ready = true;

			// Send tool input arguments
			let args: Record<string, unknown> = {};
			try {
				args = JSON.parse(toolInput);
			} catch {}
			bridge.sendToolInput({ arguments: args });

			// Send tool result
			try {
				const parsed = JSON.parse(toolOutput);
				// If it looks like a CallToolResult ({ content: [...] }), send as-is
				if (parsed && Array.isArray(parsed.content)) {
					bridge.sendToolResult(parsed);
				} else {
					// Wrap plain text in CallToolResult format
					bridge.sendToolResult({
						content: [{ type: "text", text: typeof parsed === "string" ? parsed : JSON.stringify(parsed) }],
					});
				}
			} catch {
				bridge.sendToolResult({
					content: [{ type: "text", text: toolOutput }],
				});
			}
		};

		// Handle size changes from the app
		bridge.onsizechange = ({ width, height }) => {
			if (!iframe) return;
			if (height !== undefined) {
				iframe.style.height = `${height}px`;
			}
			if (width !== undefined) {
				iframe.style.minWidth = `min(${width}px, 100%)`;
			}
		};

		// Handle display mode change requests (e.g. fullscreen)
		bridge.onrequestdisplaymode = async (params) => {
			const mode = params.mode === "fullscreen" ? "fullscreen" : "inline";
			fullscreen = mode === "fullscreen";
			bridge.sendHostContextChange({ displayMode: mode });
			return { mode };
		};

		// Handle open link requests
		bridge.onopenlink = async (params) => {
			window.open(params.url, "_blank", "noopener,noreferrer");
			return {};
		};

		// Connect transport — this starts the JSON-RPC handshake
		const transport = new PostMessageTransport(iframeWindow, iframeWindow);
		bridge.connect(transport);

		return () => {
			bridge.close();
		};
	});
</script>

<div class="mcp-app" class:fullscreen bind:this={container}>
	{#if fullscreen}
		<button class="mcp-app-close" onclick={() => fullscreen = false} title="Exit fullscreen">&times;</button>
	{/if}
	<div class="mcp-app-label">{toolName}</div>
	<iframe
		bind:this={iframe}
		sandbox="allow-scripts allow-same-origin"
		title={toolName}
		class="mcp-app-frame"
		class:loaded={ready}
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
		opacity: 0;
		transition: opacity 0.3s ease;
	}

	.mcp-app-frame.loaded {
		opacity: 1;
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
	}

	.fullscreen .mcp-app-label {
		padding: 0.5rem 1rem;
		margin: 0;
	}

	.fullscreen .mcp-app-frame {
		flex: 1;
		height: auto;
		border: none;
		border-radius: 0;
	}

	.mcp-app-close {
		position: absolute;
		top: 0.25rem;
		right: 0.75rem;
		z-index: 1;
		background: none;
		border: none;
		color: oklch(0.7 0 0);
		font-size: 1.5rem;
		cursor: pointer;
		line-height: 1;
		padding: 0.25rem;
	}

	.mcp-app-close:hover {
		color: oklch(0.9 0 0);
	}
</style>
