import type { User } from '$lib/server/db/schema.js';

declare global {
	namespace App {
		interface Locals {
			user?: User;
			sessionId?: string;
		}
	}
}

export {};
