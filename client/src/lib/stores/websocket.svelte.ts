import { createWebSocket } from "$lib/api/client.js";
import type { ServerEvent } from "$lib/api/types.js";

type EventHandler = (event: ServerEvent) => void;

let socket: WebSocket | null = null;
let connected = $state(false);
let reconnecting = $state(false);
let retryCount = $state(0);
let intentionalClose = false;
let retryTimer: ReturnType<typeof setTimeout> | null = null;
const handlers = new Set<EventHandler>();

const MAX_RETRY_DELAY = 30_000;

function retryDelay(): number {
	// Exponential backoff: 1s, 2s, 4s, 8s, 16s, 30s cap
	return Math.min(1000 * 2 ** retryCount, MAX_RETRY_DELAY);
}

export function getWebSocket() {
	return {
		get connected() {
			return connected;
		},
		get reconnecting() {
			return reconnecting;
		},
		get retryCount() {
			return retryCount;
		},
		connect() {
			if (socket) return;
			intentionalClose = false;

			try {
				socket = createWebSocket();
			} catch {
				scheduleReconnect();
				return;
			}

			socket.addEventListener("open", () => {
				connected = true;
				reconnecting = false;
				retryCount = 0;
			});

			socket.addEventListener("close", () => {
				connected = false;
				socket = null;
				if (!intentionalClose) {
					scheduleReconnect();
				}
			});

			socket.addEventListener("error", () => {
				// error always fires before close, so close handler will reconnect
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
			intentionalClose = true;
			if (retryTimer) {
				clearTimeout(retryTimer);
				retryTimer = null;
			}
			socket?.close();
			socket = null;
			connected = false;
			reconnecting = false;
			retryCount = 0;
		},
	};
}

function scheduleReconnect() {
	reconnecting = true;
	retryCount++;
	const delay = retryDelay();
	retryTimer = setTimeout(() => {
		retryTimer = null;
		getWebSocket().connect();
	}, delay);
}
