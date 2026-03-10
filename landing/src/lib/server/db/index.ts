import { neon } from '@neondatabase/serverless';
import { drizzle } from 'drizzle-orm/neon-http';
import * as schema from './schema.js';
import { env } from '$env/dynamic/private';

function createDb() {
	const sql = neon(env.DATABASE_URL!);
	return drizzle(sql, { schema });
}

let _db: ReturnType<typeof createDb> | null = null;

export function db() {
	if (!_db) _db = createDb();
	return _db;
}
