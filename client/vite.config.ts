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
			manifest: {
				name: "Bolly",
				short_name: "Bolly",
				description: "Your AI companion",
				theme_color: "#0d0b09",
				background_color: "#0d0b09",
				display: "standalone",
				scope: "/",
				start_url: "/",
				icons: [
					{ src: "pwa-192x192.png", sizes: "192x192", type: "image/png" },
					{ src: "pwa-512x512.png", sizes: "512x512", type: "image/png" },
					{ src: "pwa-512x512.png", sizes: "512x512", type: "image/png", purpose: "maskable" },
				],
			},
			workbox: {
				globPatterns: ["client/**/*.{js,css,ico,png,svg,webp,woff,woff2}"],
			},
		}),
	],
	server: {
		proxy: {
			"/api/ws": { target: "ws://localhost:8080", ws: true },
			"/api": "http://localhost:8080",
		},
	},
});
