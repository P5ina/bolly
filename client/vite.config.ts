import tailwindcss from "@tailwindcss/vite";
import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vite";

export default defineConfig({
	plugins: [tailwindcss(), sveltekit()],
	server: {
		proxy: {
			"/api/ws": { target: "ws://localhost:8080", ws: true },
			"/api": "http://localhost:8080",
		},
	},
});
