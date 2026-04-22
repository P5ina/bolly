import { z } from "zod";

export const EventSourceSchema = z.enum([
  "user_msg",
  "user_activity",
  "email",
  "calendar",
  "scheduled",
  "idle",
  "skill_emit",
  "instance_emit",
]);
export type EventSource = z.infer<typeof EventSourceSchema>;

export const EventSchema = z.object({
  id: z.string().min(1),
  user_id: z.string().min(1),
  source: EventSourceSchema,
  ts: z.number().int().nonnegative(),
  payload: z.record(z.unknown()).default({}),
  skill_hint: z.string().optional(),
});
export type Event = z.infer<typeof EventSchema>;

export const TriageDecisionSchema = z.enum(["ignore", "digest", "escalate"]);
export type TriageDecision = z.infer<typeof TriageDecisionSchema>;

export const TriageOutcomeSchema = z.object({
  decision: TriageDecisionSchema,
  reason: z.string(),
});
export type TriageOutcome = z.infer<typeof TriageOutcomeSchema>;

export const BudgetStateSchema = z.enum(["ok", "tight", "suppressed"]);
export type BudgetState = z.infer<typeof BudgetStateSchema>;

export const BudgetDailySchema = z.object({
  day: z.string().regex(/^\d{4}-\d{2}-\d{2}$/),
  calls: z.number().int().nonnegative().default(0),
  tokens_in: z.number().int().nonnegative().default(0),
  tokens_out: z.number().int().nonnegative().default(0),
  dollars_spent: z.number().nonnegative().default(0),
  cap_usd: z.number().positive(),
  state: BudgetStateSchema.default("ok"),
});
export type BudgetDaily = z.infer<typeof BudgetDailySchema>;

export const SkillFrontmatterSchema = z.object({
  name: z.string().min(1),
  created: z.string().optional(),
  triggers: z.array(z.record(z.unknown())).default([]),
  tools: z.array(z.string()).default([]),
  enabled: z.boolean().default(true),
});
export type SkillFrontmatter = z.infer<typeof SkillFrontmatterSchema>;

export const SkillSchema = z.object({
  frontmatter: SkillFrontmatterSchema,
  body: z.string(),
  path: z.string(),
});
export type Skill = z.infer<typeof SkillSchema>;

export const OutreachChannelSchema = z.enum(["push", "email", "digest"]);
export type OutreachChannel = z.infer<typeof OutreachChannelSchema>;

export const OutreachEntrySchema = z.object({
  id: z.string().min(1),
  ts: z.number().int().nonnegative(),
  channel: OutreachChannelSchema,
  title: z.string(),
  body: z.string().optional(),
  urgency: z.enum(["low", "medium", "high"]).default("medium"),
  delivered: z.boolean(),
  dedup_suppressed: z.boolean().default(false),
});
export type OutreachEntry = z.infer<typeof OutreachEntrySchema>;
