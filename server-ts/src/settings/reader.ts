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

// readToml<T> infers T from ZodSchema<T> as zod's *input* type, so the parsed
// result (which is actually z.output) needs a local cast. The `if (!result)`
// check handles null explicitly so the cast is never applied to a null value.
export const DEFAULT_SETTINGS: Settings = SettingsSchema.parse({}) as Settings;

/**
 * Read the instance's settings.toml; fall back to DEFAULT_SETTINGS if
 * the file does not exist. User-provided fields are merged over defaults
 * by zod's schema default propagation.
 */
export async function loadSettings(home: string, slug: string): Promise<Settings> {
  const result = await readToml(settingsFile(home, slug), SettingsSchema);
  if (!result) return DEFAULT_SETTINGS;
  return result as Settings;
}
