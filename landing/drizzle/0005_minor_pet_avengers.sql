CREATE TABLE "rate_limits" (
	"instance_id" text PRIMARY KEY NOT NULL,
	"messages_today" integer DEFAULT 0 NOT NULL,
	"tokens_this_month" integer DEFAULT 0 NOT NULL,
	"last_reset_daily" timestamp with time zone DEFAULT now() NOT NULL,
	"last_reset_monthly" timestamp with time zone DEFAULT now() NOT NULL
);
