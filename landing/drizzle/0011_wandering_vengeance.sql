ALTER TABLE "rate_limits" ADD COLUMN "rollover_4h" integer DEFAULT 0 NOT NULL;--> statement-breakpoint
ALTER TABLE "tenants" DROP COLUMN "tokens_per_4h";--> statement-breakpoint
ALTER TABLE "tenants" DROP COLUMN "tokens_per_week";