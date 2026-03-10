CREATE TYPE "public"."image_channel" AS ENUM('stable', 'nightly');--> statement-breakpoint
ALTER TABLE "tenants" ADD COLUMN "image_channel" "image_channel" DEFAULT 'stable' NOT NULL;