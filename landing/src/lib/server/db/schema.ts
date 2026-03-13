import { pgTable, text, timestamp, integer, pgEnum } from 'drizzle-orm/pg-core';

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
	messagesPerDay: integer('messages_per_day').notNull().default(150),
	tokensPerMonth: integer('tokens_per_month').notNull().default(1000000),

	createdAt: timestamp('created_at', { withTimezone: true }).notNull().defaultNow(),
	updatedAt: timestamp('updated_at', { withTimezone: true }).notNull().defaultNow(),
});

// ─── Rate Limits ────────────────────────────────────────────────────────────

export const rateLimits = pgTable('rate_limits', {
	instanceId: text('instance_id').primaryKey(),
	messagesToday: integer('messages_today').notNull().default(0),
	tokensThisMonth: integer('tokens_this_month').notNull().default(0),
	lastResetDaily: timestamp('last_reset_daily', { withTimezone: true }).notNull().defaultNow(),
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
