CREATE TABLE "google_accounts" (
	"id" text PRIMARY KEY NOT NULL,
	"user_id" text NOT NULL,
	"email" text NOT NULL,
	"access_token" text NOT NULL,
	"refresh_token" text NOT NULL,
	"expires_at" timestamp with time zone NOT NULL,
	"scopes" text NOT NULL,
	"created_at" timestamp with time zone DEFAULT now() NOT NULL
);
--> statement-breakpoint
ALTER TABLE "tenants" ALTER COLUMN "storage_limit" SET DEFAULT 10240;--> statement-breakpoint
ALTER TABLE "tenants" ALTER COLUMN "messages_per_day" SET DEFAULT 150;--> statement-breakpoint
ALTER TABLE "tenants" ALTER COLUMN "tokens_per_month" SET DEFAULT 1000000;--> statement-breakpoint
ALTER TABLE "google_accounts" ADD CONSTRAINT "google_accounts_user_id_users_id_fk" FOREIGN KEY ("user_id") REFERENCES "public"."users"("id") ON DELETE cascade ON UPDATE no action;