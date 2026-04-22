import { describe, expect, it } from "vitest";
import {
  bollyHome,
  budgetDailyFile,
  budgetDir,
  chatDir,
  conversationFile,
  instanceDir,
  outreachFile,
  sharedChannelDir,
  sharedDir,
  sharedInstancesFile,
  skillFile,
  skillsDir,
  triageFile,
} from "./paths.js";

describe("paths", () => {
  const home = "/tmp/bolly-test";

  it("bollyHome returns the passed root", () => {
    expect(bollyHome(home)).toBe(home);
  });

  it("instanceDir joins home + instances + slug", () => {
    expect(instanceDir(home, "alice")).toBe("/tmp/bolly-test/instances/alice");
  });

  it("chatDir nests under instance/chats/chatId", () => {
    expect(chatDir(home, "alice", "default")).toBe("/tmp/bolly-test/instances/alice/chats/default");
  });

  it("conversationFile lives inside chatDir", () => {
    expect(conversationFile(home, "alice", "default")).toBe(
      "/tmp/bolly-test/instances/alice/chats/default/conversation.json",
    );
  });

  it("skillsDir is instances/{slug}/skills", () => {
    expect(skillsDir(home, "alice")).toBe("/tmp/bolly-test/instances/alice/skills");
  });

  it("skillFile suffixes .md", () => {
    expect(skillFile(home, "alice", "email-check")).toBe(
      "/tmp/bolly-test/instances/alice/skills/email-check.md",
    );
  });

  it("triageFile is instances/{slug}/triage.md", () => {
    expect(triageFile(home, "alice")).toBe("/tmp/bolly-test/instances/alice/triage.md");
  });

  it("budgetDir is instances/{slug}/budget", () => {
    expect(budgetDir(home, "alice")).toBe("/tmp/bolly-test/instances/alice/budget");
  });

  it("budgetDailyFile uses YYYY-MM-DD.json", () => {
    expect(budgetDailyFile(home, "alice", "2026-04-22")).toBe(
      "/tmp/bolly-test/instances/alice/budget/2026-04-22.json",
    );
  });

  it("outreachFile is instances/{slug}/outreach.jsonl", () => {
    expect(outreachFile(home, "alice")).toBe("/tmp/bolly-test/instances/alice/outreach.jsonl");
  });

  it("sharedDir is home/shared", () => {
    expect(sharedDir(home)).toBe("/tmp/bolly-test/shared");
  });

  it("sharedChannelDir is home/shared/channel", () => {
    expect(sharedChannelDir(home)).toBe("/tmp/bolly-test/shared/channel");
  });

  it("sharedInstancesFile is home/shared/instances.json", () => {
    expect(sharedInstancesFile(home)).toBe("/tmp/bolly-test/shared/instances.json");
  });
});
