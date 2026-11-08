import { expect, open, test, type App } from "nsh/test";

async function counters(app: App) {
  let message = "";
  app.onDialog(dialog => { message = dialog.title ?? ""; return "Done"; });
  await app.keyboard.press("f8");
  if (!message.startsWith("rows=")) throw new Error("Missing bridge counters");
  return Object.fromEntries(message.split(" ").map(pair => { const [key, value] = pair.split("="); return [key, Number(value)]; }));
}

test("closing a document cancels debounce and releases exactly its own Rust session", async () => {
  const app = await open("./tests/fixtures/SearchLifecycle.tsx", { clock: {}, clipboard: "value\n" + "old\n".repeat(20_000) });
  await app.press("Paste from clipboard"); await app.press("Review import"); await app.press("Import document");
  await app.waitFor("20,000 rows · 1 column");
  await app.press("Find"); await app.paste("find-input", { text: "old" });
  await app.press("close-document");
  await app.clock!.runFor(1000);
  expect(await counters(app)).toMatchObject({ steps: 0, disposed: 1, released: 1, late: 0 });
  await app.press("Paste from clipboard"); await app.press("Review import"); await app.press("Import document");
  await app.waitFor("20,000 rows · 1 column");
  await app.press("Find"); await app.paste("find-input", { text: "old" });
  await app.clock!.runFor(500);
  await app.waitFor("1 of 20000 matches");
  await app.press("close-document"); await app.clock!.runFor(1000);
  expect(await counters(app)).toMatchObject({ disposed: 2, released: 2, late: 0 });
});

test("no-hit queries, cancellation and highlight refresh do not reload grid text", async () => {
  const app = await open("./tests/fixtures/SearchLifecycle.tsx", { width: 1280, height: 800, platform: "macos", clock: {}, clipboard: "value\nold\nother" });
  await app.press("Paste from clipboard"); await app.press("Review import"); await app.press("Import document");
  await app.waitFor("2 rows · 1 column");
  const initial = await counters(app);
  expect(initial).toMatchObject({ rows: 1, flags: 0 });
  await app.press("Find"); await app.paste("find-input", { text: "absent" });
  await app.press("Cancel search"); await app.clock!.runFor(500);
  expect(await counters(app)).toMatchObject({ rows: initial.rows, flags: 0 });
  await app.clear("find-input"); await app.paste("find-input", { text: "absent" });
  await app.clock!.runFor(500); await app.waitFor("No matches");
  expect(await counters(app)).toMatchObject({ rows: initial.rows, flags: 0 });
  await app.clear("find-input"); await app.paste("find-input", { text: "old" });
  await app.clock!.runFor(500); await app.waitFor("1 of 1 matches");
  const matched = await counters(app);
  // Revealing one match reads one preview; the 256-row text cache is untouched.
  expect(matched.rows).toBe(initial.rows + 1);
  expect(app.count("id:search-highlight")).toBe(1);
  await app.clear("find-input"); await app.paste("find-input", { text: "other" });
  await app.clock!.runFor(500); await app.waitFor("1 of 1 matches");
  const restarted = await counters(app);
  expect(restarted.rows).toBe(matched.rows + 1);
  expect(app.count("id:search-highlight")).toBe(1);
  await app.press("Close find");
  expect(app.count("id:search-highlight")).toBe(0);
  expect((await counters(app)).rows).toBe(restarted.rows);
});

test("closing with unfinished Rust search releases it and stops queued work", async () => {
  const app = await open("./tests/fixtures/SearchLifecycle.tsx", { clock: {}, clipboard: "value\n" + ("xxxxxxxx\n").repeat(300_000) });
  await app.press("Paste from clipboard");
  await app.waitUntil(() => app.enabled("Review import"), { timeout: 30000 });
  await app.press("Review import"); await app.press("Import anyway");
  await app.waitFor("300,000 rows · 1 column");
  await app.press("Find"); await app.paste("find-input", { text: "absent" });
  await app.keyboard.press("f9");
  const midway = await counters(app);
  expect(midway.unfinished).toBe(1);
  expect(midway.steps > 0).toBe(true);
  expect(app.texts().some(text => text.startsWith("Searching"))).toBe(true);
  await app.press("close-document"); await app.clock!.runFor(1000);
  expect(await counters(app)).toMatchObject({ steps: midway.steps, disposed: 1, released: 1, late: 0 });
});
