import { pgTable, text, timestamp, integer, pgEnum, unique } from 'drizzle-orm/pg-core';

export const planEnum = pgEnum('plan', ['starter', 'companion', 'unlimited']);
export const tenantStatusEnum = pgEnum('tenant_status', ['provisioning', 'running', 'stopped', 'error', 'destroyed']);
export const imageChannelEnum = pgEnum('image_channel', ['stable', 'nightly']);

// ─── Users ───────────────────────────────────────────────────────────────────

export const users = pgTable('users', {
	id: text('id').primaryKey(), // nanoid
	email: text('email').notNull().unique(),
	passwordHash: text('password_hash').notNull(),
	name: text('name'),
	stripeCustomerId: text('stripe_customer_id'),
	createdAt: timestamp('created_at', { withTimezone: true }).notNull().defaultNow(),
});

// ─── Sessions ────────────────────────────────────────────────────────────────

export const sessions = pgTable('sessions', {
	id: text('id').primaryKey(),
	userId: text('user_id').notNull().references(() => users.id, { onDelete: 'cascade' }),
	expiresAt: timestamp('expires_at', { withTimezone: true }).notNull(),
});

// ─── Password Reset Tokens ───────────────────────────────────────────────────

export const passwordResetTokens = pgTable('password_reset_tokens', {
	id: text('id').primaryKey(), // random token
	userId: text('user_id').notNull().references(() => users.id, { onDelete: 'cascade' }),
	expiresAt: timestamp('expires_at', { withTimezone: true }).notNull(),
});

// ─── Tenants (one per companion instance) ────────────────────────────────────

export const tenants = pgTable('tenants', {
	id: text('id').primaryKey(), // nanoid
	userId: text('user_id').notNull().references(() => users.id, { onDelete: 'cascade' }),
	slug: text('slug').notNull().unique(), // subdomain: {slug}.bollyai.dev
	plan: planEnum('plan').notNull().default('starter'),
	status: tenantStatusEnum('status').notNull().default('provisioning'),

	// Fly.io
	flyAppId: text('fly_app_id'),
	flyMachineId: text('fly_machine_id'),
	flyVolumeId: text('fly_volume_id'),
	flyIp: text('fly_ip'),

	// Bolly instance config
	authToken: text('auth_token'), // auto-generated, used to proxy to the instance
	shareToken: text('share_token'), // public share link token
	errorMessage: text('error_message'),
	imageChannel: imageChannelEnum('image_channel').notNull().default('stable'),

	// Stripe
	stripeSubscriptionId: text('stripe_subscription_id'),

	// Limits
	storageLimit: integer('storage_limit').notNull().default(10240), // MB (10 GB)
	maxInstances: integer('max_instances').notNull().default(1),
	tokensPerMonth: integer('tokens_per_month').notNull().default(5000000),

	// BYOK (Bring Your Own Key)
	byokProvider: text('byok_provider'), // "anthropic" | "openai" | "openrouter" | null
	byokApiKey: text('byok_api_key'),    // user's API key (plaintext, null = not BYOK)
	byokModel: text('byok_model'),       // custom model override, nullable

	createdAt: timestamp('created_at', { withTimezone: true }).notNull().defaultNow(),
	updatedAt: timestamp('updated_at', { withTimezone: true }).notNull().defaultNow(),
});

// ─── Google Accounts (OAuth tokens) ─────────────────────────────────────────

export const googleAccounts = pgTable('google_accounts', {
	id: text('id').primaryKey(), // nanoid
	tenantId: text('tenant_id').notNull().references(() => tenants.id, { onDelete: 'cascade' }),
	instanceSlug: text('instance_slug').notNull(),
	email: text('email').notNull(), // Google email address
	accessToken: text('access_token').notNull(),
	refreshToken: text('refresh_token').notNull(),
	expiresAt: timestamp('expires_at', { withTimezone: true }).notNull(),
	scopes: text('scopes').notNull(), // space-separated OAuth scopes
	createdAt: timestamp('created_at', { withTimezone: true }).notNull().defaultNow(),
}, (table) => [
	unique('uq_ga_tenant_instance_email').on(table.tenantId, table.instanceSlug, table.email),
]);

// ─── Rate Limits ────────────────────────────────────────────────────────────

export const rateLimits = pgTable('rate_limits', {
	instanceId: text('instance_id').primaryKey(),
	tokensLast4h: integer('tokens_last_4h').notNull().default(0),
	tokensThisWeek: integer('tokens_this_week').notNull().default(0),
	tokensThisMonth: integer('tokens_this_month').notNull().default(0),
	rollover4h: integer('rollover_4h').notNull().default(0), // unused tokens carried from previous 4h window
	lastReset4h: timestamp('last_reset_4h', { withTimezone: true }).notNull().defaultNow(),
	lastResetWeekly: timestamp('last_reset_weekly', { withTimezone: true }).notNull().defaultNow(),
	lastResetMonthly: timestamp('last_reset_monthly', { withTimezone: true }).notNull().defaultNow(),
});

// ─── Type exports ────────────────────────────────────────────────────────────

export type User = typeof users.$inferSelect;
export type NewUser = typeof users.$inferInsert;
export type Session = typeof sessions.$inferSelect;
export type Tenant = typeof tenants.$inferSelect;
export type NewTenant = typeof tenants.$inferInsert;
export type PasswordResetToken = typeof passwordResetTokens.$inferSelect;
export type RateLimit = typeof rateLimits.$inferSelect;
export type GoogleAccount = typeof googleAccounts.$inferSelect;
export type NewGoogleAccount = typeof googleAccounts.$inferInsert;
