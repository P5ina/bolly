<script lang="ts">
	import Nav from '$lib/components/Nav.svelte';
	import Hero from '$lib/components/Hero.svelte';
	import Features from '$lib/components/Features.svelte';
	import Demo from '$lib/components/Demo.svelte';
	import HowItWorks from '$lib/components/HowItWorks.svelte';
	import Pricing from '$lib/components/Pricing.svelte';
	import Cta from '$lib/components/Cta.svelte';
	import Footer from '$lib/components/Footer.svelte';
	import GlassSpheres from '$lib/components/GlassSpheres.svelte';
</script>

<Nav />

<GlassSpheres
	sections={[
		{ snippet: featuresSnippet, y: -18 },
		{ snippet: demoSnippet, y: -36 },
		{ snippet: howSnippet, y: -52 },
		{ snippet: pricingSnippet, y: -68 },
		{ snippet: ctaSnippet, y: -82 },
		{ snippet: footerSnippet, y: -92 },
	]}
/>

{#snippet featuresSnippet()}<Features />{/snippet}
{#snippet demoSnippet()}<Demo />{/snippet}
{#snippet howSnippet()}<HowItWorks />{/snippet}
{#snippet pricingSnippet()}<Pricing />{/snippet}
{#snippet ctaSnippet()}<Cta />{/snippet}
{#snippet footerSnippet()}<Footer />{/snippet}
