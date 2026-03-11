ALTER TABLE "tenants" ADD COLUMN "messages_per_day" integer DEFAULT 100 NOT NULL;--> statement-breakpoint
ALTER TABLE "tenants" ADD COLUMN "tokens_per_month" integer DEFAULT 500000 NOT NULL;