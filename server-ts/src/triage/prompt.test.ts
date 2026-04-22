import { describe, expect, it } from "vitest";
import type { Event } from "../types.js";
import { buildTriagePrompt } from "./prompt.js";

const EVENT: Event = {
  id: "01JKM000000000000000000000",
  user_id: "alice",
  source: "email",
  ts: 1714000000000,
  payload: { subject: "Q2 review", from: "boss@corp.com" },
};

describe("buildTriagePrompt", () => {
  it("includes soul, mood, budget, triage rules, and the event", () => {
    const prompt = buildTriagePrompt({
      soulSnippet: "Bolly is calm and attentive.",
      mood: "focused",
      budgetState: "ok",
      dollarsSpent: 0.42,
      capUsd: 2.0,
      triageRules: "# rules\n- urgent email -> escalate",
      event: EVENT,
      recentOutreach: [],
    });

    expect(prompt).toContain("<soul>Bolly is calm and attentive.</soul>");
    expect(prompt).toContain("<mood>focused</mood>");
    expect(prompt).toContain("0.42/2.00");
    expect(prompt).toContain("ok");
    expect(prompt).toContain("urgent email -> escalate");
    expect(prompt).toContain("Q2 review");
  });

  it("adds tightening directive when state is tight", () => {
    const prompt = buildTriagePrompt({
      soulSnippet: "",
      mood: "",
      budgetState: "tight",
      dollarsSpent: 1.5,
      capUsd: 2.0,
      triageRules: "",
      event: EVENT,
      recentOutreach: [],
    });
    expect(prompt).toContain("Only escalate if truly urgent");
  });

  it("adds suppression directive when state is suppressed", () => {
    const prompt = buildTriagePrompt({
      soulSnippet: "",
      mood: "",
      budgetState: "suppressed",
      dollarsSpent: 2.0,
      capUsd: 2.0,
      triageRules: "",
      event: EVENT,
      recentOutreach: [],
    });
    expect(prompt).toContain("budget cap reached");
  });

  it("includes the last N outreach entries for self-regulation context", () => {
    const prompt = buildTriagePrompt({
      soulSnippet: "",
      mood: "",
      budgetState: "ok",
      dollarsSpent: 0,
      capUsd: 2.0,
      triageRules: "",
      event: EVENT,
      recentOutreach: [
        { channel: "push", title: "reminder", ts: 1713999999000, urgency: "medium" },
        { channel: "email", title: "digest", ts: 1713999998000, urgency: "low" },
      ],
    });
    expect(prompt).toContain("push: reminder");
    expect(prompt).toContain("email: digest");
  });

  it("asks for the single-line response format", () => {
    const prompt = buildTriagePrompt({
      soulSnippet: "",
      mood: "",
      budgetState: "ok",
      dollarsSpent: 0,
      capUsd: 2.0,
      triageRules: "",
      event: EVENT,
      recentOutreach: [],
    });
    expect(prompt).toMatch(/DECISION=.*REASON=/);
  });

  it("truncates large event payloads", () => {
    const bigEvent: Event = {
      ...EVENT,
      payload: { body: "x".repeat(5000) },
    };
    const prompt = buildTriagePrompt({
      soulSnippet: "",
      mood: "",
      budgetState: "ok",
      dollarsSpent: 0,
      capUsd: 2.0,
      triageRules: "",
      event: bigEvent,
      recentOutreach: [],
    });
    expect(prompt.length).toBeLessThan(6000);
    expect(prompt).toContain("[truncated]");
  });
});
