import { z } from "zod";
import { settingsFile } from "../paths.js";
import { readToml } from "../toml-file.js";

const SettingsSchema = z.object({
  daily_budget_usd: z.number().positive().default(2.0),
  quiet_hours: z
    .object({
      start: z.string().default("22:00"),
      end: z.string().default("07:00"),
    })
    .default({}),
  push: z
    .object({
      enabled: z.boolean().default(true),
      daily_max: z.number().int().positive().default(5),
    })
    .default({}),
  email: z
    .object({
      enabled: z.boolean().default(true),
      address: z.string().optional(),
      daily_max: z.number().int().positive().default(2),
    })
    .default({}),
});

// z.output gives the fully-resolved shape after defaults are applied
export type Settings = z.output<typeof SettingsSchema>;

// Cast required: TS infers the input type from parse({}); at runtime zod fills
// all defaults so the cast is sound.
export const DEFAULT_SETTINGS = SettingsSchema.parse({}) as unknown as Settings;

/**
 * Read the instance's settings.toml; fall back to DEFAULT_SETTINGS if
 * the file does not exist. User-provided fields are merged over defaults
 * by zod's schema default propagation.
 */
export async function loadSettings(home: string, slug: string): Promise<Settings> {
  const result = await readToml(settingsFile(home, slug), SettingsSchema);
  return (result as unknown as Settings) ?? DEFAULT_SETTINGS;
}
