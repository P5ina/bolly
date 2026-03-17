ALTER TABLE "tenants" ALTER COLUMN "tokens_per_month" SET DEFAULT 5000000;--> statement-breakpoint
ALTER TABLE "rate_limits" ADD COLUMN "tokens_last_4h" integer DEFAULT 0 NOT NULL;--> statement-breakpoint
ALTER TABLE "rate_limits" ADD COLUMN "tokens_this_week" integer DEFAULT 0 NOT NULL;--> statement-breakpoint
ALTER TABLE "rate_limits" ADD COLUMN "last_reset_4h" timestamp with time zone DEFAULT now() NOT NULL;--> statement-breakpoint
ALTER TABLE "rate_limits" ADD COLUMN "last_reset_weekly" timestamp with time zone DEFAULT now() NOT NULL;--> statement-breakpoint
ALTER TABLE "tenants" ADD COLUMN "tokens_per_4h" integer DEFAULT 200000 NOT NULL;--> statement-breakpoint
ALTER TABLE "tenants" ADD COLUMN "tokens_per_week" integer DEFAULT 2000000 NOT NULL;--> statement-breakpoint
ALTER TABLE "rate_limits" DROP COLUMN "messages_today";--> statement-breakpoint
ALTER TABLE "rate_limits" DROP COLUMN "last_reset_daily";--> statement-breakpoint
ALTER TABLE "tenants" DROP COLUMN "messages_per_day";