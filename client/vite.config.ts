import tailwindcss from "@tailwindcss/vite";
import { sveltekit } from "@sveltejs/kit/vite";
import { SvelteKitPWA } from "@vite-pwa/sveltekit";
import { defineConfig } from "vite";

export default defineConfig({
	plugins: [
		tailwindcss(),
		sveltekit(),
		SvelteKitPWA({
			registerType: "autoUpdate",
			manifest: false,
			workbox: {
				globPatterns: ["client/**/*.{js,css,ico,png,svg,webp,woff,woff2}"],
				navigateFallbackDenylist: [/^\/api/, /^\/auth/, /^\/manifest\.webmanifest/],
			},
		}),
	],
	server: {
		proxy: {
			"/api/ws": { target: "ws://localhost:8080", ws: true },
			"/api": "http://localhost:8080",
			"/manifest.webmanifest": "http://localhost:8080",
		},
	},
});
