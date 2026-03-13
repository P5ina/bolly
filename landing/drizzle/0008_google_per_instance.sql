-- Move Google OAuth from user-level to instance-level (tenant + instanceSlug)
ALTER TABLE "google_accounts" ADD COLUMN "tenant_id" text;--> statement-breakpoint
ALTER TABLE "google_accounts" ADD COLUMN "instance_slug" text NOT NULL DEFAULT 'default';--> statement-breakpoint
-- Populate tenant_id from userId → first tenant for that user
UPDATE "google_accounts" ga SET "tenant_id" = (
  SELECT t.id FROM "tenants" t WHERE t.user_id = ga.user_id LIMIT 1
);--> statement-breakpoint
-- Drop rows with no matching tenant
DELETE FROM "google_accounts" WHERE "tenant_id" IS NULL;--> statement-breakpoint
-- Now make tenant_id NOT NULL
ALTER TABLE "google_accounts" ALTER COLUMN "tenant_id" SET NOT NULL;--> statement-breakpoint
-- Drop old userId FK and column
ALTER TABLE "google_accounts" DROP CONSTRAINT IF EXISTS "google_accounts_user_id_users_id_fk";--> statement-breakpoint
ALTER TABLE "google_accounts" DROP COLUMN "user_id";--> statement-breakpoint
-- Add new FK and unique constraint
ALTER TABLE "google_accounts" ADD CONSTRAINT "google_accounts_tenant_id_tenants_id_fk" FOREIGN KEY ("tenant_id") REFERENCES "public"."tenants"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
CREATE UNIQUE INDEX "uq_ga_tenant_instance_email" ON "google_accounts" ("tenant_id", "instance_slug", "email");
