import { createWebSocket } from "$lib/api/client.js";
import type { ServerEvent } from "$lib/api/types.js";

type EventHandler = (event: ServerEvent) => void;

let socket: WebSocket | null = null;
let connected = $state(false);
const handlers = new Set<EventHandler>();

export function getWebSocket() {
	return {
		get connected() {
			return connected;
		},
		connect() {
			if (socket) return;

			socket = createWebSocket();

			socket.addEventListener("open", () => {
				connected = true;
			});

			socket.addEventListener("close", () => {
				connected = false;
				socket = null;
				setTimeout(() => this.connect(), 3000);
			});

			socket.addEventListener("message", (ev) => {
				try {
					const event: ServerEvent = JSON.parse(ev.data);
					for (const handler of handlers) {
						handler(event);
					}
				} catch {
					// ignore malformed messages
				}
			});
		},
		subscribe(handler: EventHandler): () => void {
			handlers.add(handler);
			return () => handlers.delete(handler);
		},
		disconnect() {
			socket?.close();
			socket = null;
			connected = false;
		},
	};
}
