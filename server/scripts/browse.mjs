#!/usr/bin/env node
// Browser automation script for the browse tool.
// Reads a JSON payload from stdin, executes Playwright actions, returns JSON on stdout.
//
// Input:  { actions: [{action, url?, selector?, text?, ms?, script?}], timeout?: number }
// Output: { ok: bool, results: [{action, ok, ...}] }

import { chromium } from "playwright-core";

const MAX_CONTENT = 12000;
const DEFAULT_TIMEOUT = 60000;
const ACTION_TIMEOUT = 30000;

async function main() {
  const input = await new Promise((resolve, reject) => {
    let data = "";
    process.stdin.setEncoding("utf8");
    process.stdin.on("data", (chunk) => (data += chunk));
    process.stdin.on("end", () => {
      try { resolve(JSON.parse(data)); }
      catch (e) { reject(new Error(`Invalid JSON input: ${e.message}`)); }
    });
    process.stdin.on("error", reject);
  });

  const actions = input.actions || [];
  if (!actions.length) {
    console.log(JSON.stringify({ ok: false, error: "No actions provided", results: [] }));
    process.exit(1);
  }

  const totalTimeout = Math.min(input.timeout || DEFAULT_TIMEOUT, 120000);
  const deadline = Date.now() + totalTimeout;

  const browser = await chromium.launch({
    executablePath: process.env.CHROMIUM_PATH || undefined,
    args: [
      "--no-sandbox",
      "--disable-gpu",
      "--disable-dev-shm-usage",
      "--disable-extensions",
      "--disable-background-networking",
      "--renderer-process-limit=1",
      "--single-process",
    ],
  });

  const results = [];
  try {
    const context = await browser.newContext({
      viewport: { width: 1280, height: 720 },
      userAgent: "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    });
    const page = await context.newPage();
    page.setDefaultTimeout(ACTION_TIMEOUT);

    for (const act of actions) {
      if (Date.now() > deadline) {
        results.push({ action: act.action, ok: false, error: "Total timeout exceeded" });
        break;
      }

      try {
        switch (act.action) {
          case "navigate": {
            const resp = await page.goto(act.url, { waitUntil: "domcontentloaded" });
            results.push({
              action: "navigate",
              ok: true,
              url: page.url(),
              title: await page.title(),
              status: resp?.status(),
            });
            break;
          }

          case "content": {
            let text = await page.innerText("body").catch(() => "");
            // Collapse whitespace
            text = text.replace(/\n{3,}/g, "\n\n").replace(/[ \t]+/g, " ").trim();
            if (text.length > MAX_CONTENT) text = text.slice(0, MAX_CONTENT) + "\n...(truncated)";
            results.push({ action: "content", ok: true, text });
            break;
          }

          case "screenshot": {
            const path = act.path || "/tmp/screenshot.png";
            await page.screenshot({ path, fullPage: !!act.full_page });
            results.push({ action: "screenshot", ok: true, path });
            break;
          }

          case "click": {
            await page.click(act.selector);
            results.push({ action: "click", ok: true, selector: act.selector });
            break;
          }

          case "type": {
            await page.fill(act.selector, act.text || "");
            results.push({ action: "type", ok: true, selector: act.selector });
            break;
          }

          case "wait": {
            const ms = Math.min(act.ms || 1000, 10000);
            await page.waitForTimeout(ms);
            results.push({ action: "wait", ok: true, ms });
            break;
          }

          case "evaluate": {
            const value = await page.evaluate(act.script);
            const str = typeof value === "string" ? value : JSON.stringify(value);
            results.push({ action: "evaluate", ok: true, value: str?.slice(0, 4000) });
            break;
          }

          case "select": {
            await page.selectOption(act.selector, act.text || "");
            results.push({ action: "select", ok: true, selector: act.selector });
            break;
          }

          default:
            results.push({ action: act.action, ok: false, error: `Unknown action: ${act.action}` });
        }
      } catch (err) {
        results.push({ action: act.action, ok: false, error: err.message?.slice(0, 500) });
        // Stop on error — partial results are returned
        break;
      }
    }
  } finally {
    await browser.close().catch(() => {});
  }

  const allOk = results.every((r) => r.ok);
  console.log(JSON.stringify({ ok: allOk, results }));
  process.exit(allOk ? 0 : 1);
}

main().catch((err) => {
  console.log(JSON.stringify({ ok: false, error: err.message, results: [] }));
  process.exit(1);
});
