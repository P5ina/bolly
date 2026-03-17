ALTER TABLE "tenants" ALTER COLUMN "tokens_per_month" SET DEFAULT 1000000;--> statement-breakpoint
ALTER TABLE "tenants" ADD COLUMN "tokens_per_4h" integer DEFAULT 100000 NOT NULL;--> statement-breakpoint
ALTER TABLE "rate_limits" DROP COLUMN "rollover_4h";