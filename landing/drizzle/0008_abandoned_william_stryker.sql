ALTER TABLE "google_accounts" DROP CONSTRAINT "google_accounts_user_id_users_id_fk";
--> statement-breakpoint
ALTER TABLE "google_accounts" ADD COLUMN "tenant_id" text NOT NULL;--> statement-breakpoint
ALTER TABLE "google_accounts" ADD COLUMN "instance_slug" text NOT NULL;--> statement-breakpoint
ALTER TABLE "google_accounts" ADD CONSTRAINT "google_accounts_tenant_id_tenants_id_fk" FOREIGN KEY ("tenant_id") REFERENCES "public"."tenants"("id") ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE "google_accounts" DROP COLUMN "user_id";--> statement-breakpoint
ALTER TABLE "google_accounts" ADD CONSTRAINT "uq_ga_tenant_instance_email" UNIQUE("tenant_id","instance_slug","email");