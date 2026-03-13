<script lang="ts">
	let {
		scene,
		height = "300px",
	}: {
		scene: string;
		height?: string;
	} = $props();

	let container: HTMLDivElement;
	let root: any = null;
	let loaded = $state(false);
	let error = $state<string | null>(null);

	function parseScene(raw: string) {
		try {
			const data = JSON.parse(raw);
			return {
				elements: data.elements ?? [],
				appState: {
					theme: "dark",
					...(data.appState ?? {}),
					viewBackgroundColor: "transparent",
				},
				files: data.files ?? undefined,
			};
		} catch {
			return null;
		}
	}

	async function mount(node: HTMLDivElement, sceneStr: string) {
		try {
			const [React, ReactDOM, ExcalidrawModule] = await Promise.all([
				import("react"),
				import("react-dom/client"),
				import("@excalidraw/excalidraw"),
			]);

			const parsed = parseScene(sceneStr);
			if (!parsed) {
				error = "invalid scene data";
				return;
			}

			const { Excalidraw } = ExcalidrawModule;

			const element = React.createElement(Excalidraw, {
				initialData: parsed,
				viewModeEnabled: true,
				zenModeEnabled: true,
				gridModeEnabled: false,
				theme: "dark",
				UIOptions: {
					canvasActions: {
						changeViewBackgroundColor: false,
						clearCanvas: false,
						export: false,
						loadScene: false,
						saveToActiveFile: false,
						toggleTheme: false,
					},
				},
			});

			root = ReactDOM.createRoot(node);
			root.render(element);
			loaded = true;
		} catch (e) {
			error = e instanceof Error ? e.message : "failed to load excalidraw";
		}
	}

	$effect(() => {
		if (container && scene) {
			mount(container, scene);
		}

		return () => {
			if (root) {
				root.unmount();
				root = null;
			}
		};
	});
</script>

<div class="excalidraw-wrap" style="height: {height}">
	{#if error}
		<div class="excalidraw-error">{error}</div>
	{:else if !loaded}
		<div class="excalidraw-loading">loading sketch...</div>
	{/if}
	<div
		bind:this={container}
		class="excalidraw-container"
		class:excalidraw-hidden={!loaded}
	></div>
</div>

<style>
	.excalidraw-wrap {
		position: relative;
		width: 100%;
		border-radius: 0.75rem;
		overflow: hidden;
		background: oklch(0.06 0.01 280);
		border: 1px solid oklch(0.78 0.12 75 / 10%);
	}

	.excalidraw-container {
		width: 100%;
		height: 100%;
	}

	.excalidraw-hidden {
		opacity: 0;
	}

	/* Override Excalidraw's internal styles for dark theme */
	.excalidraw-container :global(.excalidraw) {
		--color-surface-lowest: transparent !important;
		--color-surface-low: transparent !important;
		--island-bg-color: oklch(0.12 0.015 280) !important;
	}

	.excalidraw-container :global(.excalidraw .Island) {
		display: none !important;
	}

	.excalidraw-container :global(.excalidraw .layer-ui__wrapper__footer) {
		display: none !important;
	}

	.excalidraw-container :global(.excalidraw .main-menu-trigger) {
		display: none !important;
	}

	.excalidraw-loading,
	.excalidraw-error {
		position: absolute;
		inset: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		font-family: var(--font-mono);
		font-size: 0.7rem;
		color: oklch(0.78 0.12 75 / 30%);
	}

	.excalidraw-error {
		color: oklch(0.65 0.15 25 / 60%);
	}
</style>
