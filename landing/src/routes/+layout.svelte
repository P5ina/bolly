<script lang="ts">
	import '../app.css';

	let { children } = $props();
</script>

<svelte:head>
	<title>Bolly — A companion that lives on your server</title>
</svelte:head>

{@render children()}
