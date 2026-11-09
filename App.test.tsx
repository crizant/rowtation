import { expect, open, test } from "nsh/test";

test("paste import keeps literal values, inspects columns and closes the session", async () => {
  const app = await open("./App.tsx", { width: 1280, height: 800 });
  await app.press("Paste from clipboard");
  await app.paste("paste-input", { text: "id,city\n00123,Brisbane\n00234,Sydney\n00345,Brisbane" });
  await app.press("Review import");
  await app.press("Import document");
  await app.waitFor("3 rows · 2 columns");
  expect(app.texts()).toContain("00123");
  await app.press("Brisbane >> nth=0");
  expect(app.shows("id:inspector-panel")).toBe(true);
  await app.press("copy-value");
  await app.waitUntil(() => app.clipboard() === "Brisbane");
  await app.press("close-document");
  expect(app.shows("id:home-scroll")).toBe(true);
  expect(app.texts()).not.toContain("00123");
});

test("back returns home before another import and failed imports can be retried", async () => {
  const app = await open("./App.tsx", { clipboard: "name\nOriginal" });
  await app.press("Paste from clipboard");
  await app.press("Review import");
  await app.press("Import document");
  await app.waitFor("1 row · 1 column");
  await app.press("close-document");
  await app.press("Paste from clipboard");
  await app.paste("paste-input", { text: '\n"unclosed' });
  await app.press("Review import");
  await app.press("Import document");
  expect(app.texts().join("\n")).toContain("INVALID_CSV");
  await app.press("Cancel");
  expect(app.shows("id:home-scroll")).toBe(true);
  expect(app.texts()).not.toContain("Original");
  await app.press("Paste from clipboard");
  await app.clear("paste-input");
  await app.paste("paste-input", { text: "name\nReplacement" });
  await app.press("Review import");
  await app.press("Import document");
  await app.waitFor("Replacement");
  expect(app.texts()).not.toContain("Original");
});

test("mobile TSV import without headers and contextual inspector", async () => {
  const app = await open("./App.tsx", { width: 390, height: 844 });
  await app.press("Paste from clipboard");
  await app.paste("paste-input", { text: "001\tMia\n002\tOliver" });
  await app.press("Review import");
  expect(app.texts()).toContain("Tab");
  await app.press("First row contains column headers");
  await app.press("Import document");
  await app.waitFor("2 rows · 2 columns");
  expect(app.texts()).toContain("Column 1");
  await app.press("001");
  await app.press("Column info");
  await app.press("copy-value");
  await app.waitUntil(() => app.clipboard() === "001");
  await app.press("Done");
  await app.setViewport(1280, 800);
  expect(app.shows("id:inspector-panel")).toBe(true);
});

test("virtualized row windows and keyboard selection survive offscreen jumps", async () => {
  const text = "id,city\n" + Array.from({ length: 2000 }, (_, i) => `${i + 1},City ${i + 1}`).join("\n");
  const app = await open("./App.tsx", { width: 1280, height: 800, clipboard: text });
  await app.press("Paste from clipboard");
  await app.press("Review import");
  await app.press("Import document");
  await app.waitFor("2,000 rows · 2 columns");
  expect(app.texts()).not.toContain("City 2000");
  await app.press("City 1");
  await app.keyboard.press("arrowdown");
  expect(app.texts()).toContain("Row 2 · Column 2");
  await app.scroll("data-grid", 2000);
  expect(app.texts()).not.toContain("City 2000");
  // Selection is identity-based, retained while its row is recycled offscreen.
  expect(app.texts()).toContain("Row 2 · Column 2");
  await app.scroll("data-grid", -2000);
  await app.keyboard.press("arrowdown");
  expect(app.texts()).toContain("Row 3 · Column 2");
  // Page navigation crosses the cache without changing the selected column.
  for (let page = 0; page < 20; page++) await app.keyboard.press("pagedown");
  const selected = app.texts().find(text => /^Row \d+ · Column 2$/.test(text));
  const row = Number(selected?.split(" ")[1]);
  expect(row > 256).toBe(true);
  expect(app.texts()).toContain(`City ${row}`);
  for (let page = 0; page < 21; page++) await app.keyboard.press("pageup");
  expect(app.texts()).toContain("Row 1 · Column 2");
  await app.keyboard.press("arrowdown");
  expect(app.texts()).toContain("Row 2 · Column 2");
});

test("app runtime creates, queries, serializes and disposes the Rust document", async () => {
  const app = await open("./tests/fixtures/DocumentSmoke.tsx");
  expect(app.texts()).toContain("1 row, 1 column: 00123");
  expect(app.texts()).toContain("Bridge checks passed");
  await app.press("Close document");
  expect(app.texts()).toContain("Disposed");
});

test("wide tables split tall viewport reads to respect Rust cell-window limits", async () => {
  const header = Array.from({ length: 512 }, (_, column) => `Field ${column}`).join(",");
  const rows = Array.from({ length: 60 }, (_, row) => Array.from({ length: 512 }, (_, column) => `r${row}c${column}`).join(",")).join("\n");
  const app = await open("./App.tsx", { width: 1280, height: 2160, clipboard: `${header}\n${rows}` });
  await app.press("Paste from clipboard");
  await app.press("Review import");
  await app.press("Import document");
  await app.waitFor("60 rows · 512 columns");
  expect(app.texts()).toContain("r0c0");
  expect(app.texts()).not.toContain("Field 511");
  await app.scroll("data-grid", 1000);
  await app.waitFor("r59c0");
  await app.press("r59c0");
  await app.setViewport(390, 844);
  for (let column = 0; column < 9; column++) await app.keyboard.press("arrowright");
  await app.waitFor("Field 9");
  expect(app.texts()).toContain("r59c9");
});

test("large data imports only after an explicit confirmation", async () => {
  const text = "id\n" + Array.from({ length: 100_001 }, (_, i) => String(i + 1)).join("\n");
  const app = await open("./App.tsx", { width: 1280, height: 800, clipboard: text });
  await app.press("Paste from clipboard");
  await app.waitUntil(() => app.value("paste-input") === text);
  await app.press("Review import");
  await app.waitFor("This is a large dataset");
  expect(app.texts().join("\n")).toContain("100,001 rows · 1 column");
  expect(app.texts()).not.toContain("Import document");
  await app.press("Cancel");
  expect(app.shows("id:home-scroll")).toBe(true);
  await app.press("Paste from clipboard");
  await app.waitUntil(() => app.value("paste-input") === text);
  await app.press("Review import");
  await app.press("Import anyway");
  await app.waitFor("100,001 rows · 1 column");
});

test("small data imports without a warning", async () => {
  const app = await open("./App.tsx", { clipboard: "id\n1" });
  await app.press("Paste from clipboard");
  await app.press("Review import");
  expect(app.texts()).not.toContain("This is a large dataset");
  expect(app.texts()).toContain("Import document");
});

test("long values show previews in the grid and whole values on request", async () => {
  const long = "雪".repeat(40_000) + "END";
  const app = await open("./App.tsx", { width: 1280, height: 800, clipboard: `id,note\n007,${long}` });
  await app.press("Paste from clipboard");
  await app.press("Review import");
  await app.press("Import document");
  await app.waitFor("1 row · 2 columns");
  expect(app.texts()).toContain("雪".repeat(170) + "…");
  expect(app.texts().join("\n")).not.toContain("END");
  await app.press("007");
  await app.keyboard.press("arrowright");
  await app.waitFor("117 KB · preview");
  await app.press("copy-value");
  await app.wait(100);
  expect(app.clipboard()).toBe(long);
  await app.press("View full value");
  await app.waitFor("id:full-value");
  expect(app.texts()).toContain(long.slice(0, 32_768));
  expect(app.texts()).not.toContain(long);
  await app.press("Show more");
  await app.waitFor(long);
  await app.press("copy-full-value");
  await app.waitFor("Copied");
  expect(app.clipboard()).toBe(long);
});

test("diagnostics are off until requested and stay in the tab", async () => {
  const app = await open("./App.tsx", { width: 1280, height: 800, clipboard: "id\n1\n2" });
  expect(app.shows("id:diagnostics")).toBe(false);
  await app.keyboard.press("ctrl+shift+d");
  await app.waitFor("id:diagnostics");
  await app.press("Paste from clipboard");
  await app.press("Review import");
  await app.press("Import document");
  await app.waitFor("2 rows · 1 column");
  const log = app.texts().join("\n");
  expect(log).toContain("Inspect: ");
  expect(log).toContain("Parse: ");
  expect(log).toContain("Row window: ");
  await app.keyboard.press("ctrl+shift+d");
  expect(app.shows("id:diagnostics")).toBe(false);
});


test("long pasted text stays inside the field and import actions remain reachable in short dialogs", async () => {
  const app = await open("./App.tsx", { width: 1066, height: 688 });
  await app.press("Paste from clipboard");
  await app.paste("paste-input", { text: "id,city\n" + Array.from({ length: 100 }, (_, i) => `${i + 1},Brisbane`).join("\n") });
  expect(app.shows("id:paste-input")).toBe(true);
  await app.setViewport(390, 360);
  expect(app.shows("id:paste-input")).toBe(true);
  await app.press("Review import");
  expect(app.shows("id:import-summary")).toBe(true);
  await app.press("Import document");
  await app.waitFor("100 rows · 2 columns");
});

test("search navigation and scoped replacement remain correct after sort", async () => {
  const app = await open("./App.tsx", { width: 1280, height: 800, clipboard: "id,city\nb,Brisbane Brisbane\na,Sydney\nc,Brisbane" });
  await app.press("Paste from clipboard"); await app.press("Review import"); await app.press("Import document");
  await app.waitFor("3 rows · 2 columns");
  await app.press("Sydney"); await app.press("Find");
  expect(app.shows("id:replace-input")).toBe(false);
  await app.paste("find-input", { text: "Brisbane" }); await app.waitFor("1 of 3 matches");
  expect(app.count("id:search-highlight") > 0).toBe(true);
  await app.press("Next"); await app.waitFor("2 of 3 matches");
  await app.press("Next"); await app.waitFor("Row 3 · Column 2");
  await app.press("Previous"); await app.waitFor("2 of 3 matches");
  await app.press("Replace…");
  await app.paste("replace-input", { text: "City" }); await app.press("Replace");
  await app.waitFor("1 replacement in 1 cell");
  await app.press("Replace all"); await app.waitFor("2 replacements in 2 cells");
  await app.press("Undo"); await app.waitFor("2 of 2 matches");
  await app.press("Undo"); await app.waitFor("3 of 3 matches");
  await app.press("Close find");
  await app.waitUntil(() => app.count("id:search-highlight") === 0);
  await app.press("a"); await app.press("id:sort:1"); await app.press("id:sort:1");
  await app.press("c"); await app.keyboard.press("f2");
  await app.clear("cell-draft"); await app.paste("cell-draft", { text: "z" }); await app.press("Save cell");
  await app.waitFor("1 modified · Unsaved");
  await app.press("id:sort:1"); await app.press("z"); await app.waitFor("Row 3 · Column 1");
  await app.press("Undo"); await app.waitFor("Unmodified");
});

test("mobile touch editing supports commit cancel and undo", async () => {
  const app = await open("./App.tsx", { width: 390, height: 844, clipboard: "name,city\nMia,Brisbane" });
  await app.press("Paste from clipboard"); await app.press("Review import"); await app.press("Import document");
  await app.waitFor("1 row · 2 columns"); await app.press("Mia");
  await app.wait(600); await app.press("Mia"); await app.press("Mia");
  await app.clear("cell-draft"); await app.paste("cell-draft", { text: "Ann" }); await app.press("Save cell");
  await app.waitFor("Ann"); await app.wait(600); await app.press("Ann"); await app.press("Ann");
  await app.paste("cell-draft", { text: "discard" }); await app.press("Cancel");
  await app.press("Undo"); await app.waitFor("Mia"); await app.waitFor("Unmodified");
  await app.press("Redo"); await app.waitFor("Ann");
});

test("cell editor keyboard commits, tabs, and cancels without mutating drafts", async () => {
  const app = await open("./App.tsx", { width: 1280, height: 800, clipboard: "a,b\nx,y" });
  await app.press("Paste from clipboard"); await app.press("Review import"); await app.press("Import document");
  await app.waitFor("1 row · 2 columns"); await app.press("x"); await app.keyboard.press("f2");
  await app.clear("cell-draft"); await app.paste("cell-draft", {text: "first"}); await app.keyboard.press("tab");
  await app.waitFor("Row 1 · Column 2"); await app.keyboard.press("f2");
  await app.clear("cell-draft"); await app.paste("cell-draft", {text: "second"}); await app.keyboard.press("enter");
  await app.waitFor("2 modified · Unsaved"); await app.keyboard.press("f2");
  await app.paste("cell-draft", {text: "discard"}); await app.keyboard.press("escape");
  await app.waitFor("second"); await app.keyboard.press("mod+z"); await app.waitFor("1 modified · Unsaved");
  await app.keyboard.press("mod+shift+z"); await app.waitFor("2 modified · Unsaved");
  await app.keyboard.press("mod+f");
  expect(app.shows("id:find-input")).toBe(true);
  expect(app.shows("id:replace-input")).toBe(false);
});

test("Back asks before discarding a modified document", async () => {
  const app = await open("./App.tsx", { width: 1280, height: 800, clipboard: "id,city\n1,Sydney" });
  await app.press("Paste from clipboard"); await app.press("Review import"); await app.press("Import document");
  await app.waitFor("1 row · 2 columns");
  await app.press("Sydney"); await app.keyboard.press("f2");
  await app.clear("cell-draft"); await app.paste("cell-draft", { text: "Melbourne" }); await app.press("Save cell");
  await app.waitFor("1 modified · Unsaved");
  let answer = "Keep editing";
  app.onDialog(() => answer);
  await app.press("close-document");
  expect(app.texts()).toContain("Melbourne");
  expect(app.shows("id:paste-input")).toBe(false);
  answer = "Discard changes";
  await app.press("close-document");
  expect(app.shows("id:home-scroll")).toBe(true);
  expect(app.texts()).not.toContain("Melbourne");
});

test("copied search matches stay plain cell text", async () => {
  const app = await open("./App.tsx", { width: 1280, height: 800, platform: "macos", clipboard: "id,city\nb,Brisbane Brisbane\na,Sydney" });
  await app.press("Paste from clipboard"); await app.press("Review import"); await app.press("Import document");
  await app.waitFor("2 rows · 2 columns");
  await app.press("Find"); await app.paste("find-input", { text: "Brisbane" });
  await app.waitFor("1 of 2 matches");
  await app.keyboard.press("tab");
  expect(app.count("id:search-highlight") > 0).toBe(true);
  await app.press("Brisbane Brisbane");
  await app.keyboard.press("mod+c");
  expect(app.clipboard()).toBe("Brisbane Brisbane");
  await app.press("copy-value");
  await app.waitFor("Copied");
  expect(app.clipboard()).toBe("Brisbane Brisbane");
});

test("double-press edits the cell under the pointer", async () => {
  const app = await open("./App.tsx", { width: 1280, height: 800, clipboard: "id,city\na,Sydney\nb,Brisbane" });
  await app.press("Paste from clipboard"); await app.press("Review import"); await app.press("Import document");
  await app.waitFor("2 rows · 2 columns");
  await app.press("Sydney");
  await app.wait(600);
  await app.press("a"); await app.press("a");
  await app.waitFor("Edit id");
  expect(app.value("cell-draft")).toBe("a");
});

test("a cell menu can edit the pressed cell", async () => {
  const app = await open("./App.tsx", { width: 1280, height: 800, platform: "macos", clipboard: "id,city\na,Sydney" });
  await app.press("Paste from clipboard"); await app.press("Review import"); await app.press("Import document");
  await app.waitFor("1 row · 2 columns");
  await app.rightClick("Sydney");
  await app.press("Edit cell");
  await app.waitFor("Edit city");
  expect(app.value("cell-draft")).toBe("Sydney");
});

test("mobile find keeps the query separate from replacement", async () => {
  const app = await open("./App.tsx", { width: 390, height: 844, clipboard: "name,city\nMia,Brisbane" });
  await app.press("Paste from clipboard"); await app.press("Review import"); await app.press("Import document");
  await app.waitFor("1 row · 2 columns");
  expect(app.texts()).toContain("Pasted data");
  await app.press("Find");
  expect(app.shows("id:find-input")).toBe(true);
  expect(app.shows("id:replace-input")).toBe(false);
  expect(app.texts()).toContain("All columns");
  expect(app.texts()).not.toContain("0 matches");
  await app.press("Replace…");
  expect(app.shows("id:replace-input")).toBe(true);
  await app.press("Hide replace");
  expect(app.shows("id:replace-input")).toBe(false);
  await app.press("Close find");
  await app.wait(600);
  await app.press("Mia"); await app.press("Mia");
  await app.clear("cell-draft"); await app.paste("cell-draft", { text: "Ann" }); await app.press("Save cell");
  await app.waitFor("1 modified · Unsaved");
  let answer = "Keep editing";
  app.onDialog(() => answer);
  await app.press("close-document");
  expect(app.shows("id:paste-input")).toBe(false);
  answer = "Discard changes";
  await app.press("close-document");
  expect(app.shows("id:home-scroll")).toBe(true);
  expect(app.shows("id:paste-input")).toBe(false);
  await app.press("Paste from clipboard");
  expect(app.shows("id:paste-input")).toBe(true);
});

test("find keyboard drives the open query and restores the grid", async () => {
  const app = await open("./App.tsx", { width: 1280, height: 800, clipboard: "id,city\n1,Brisbane Brisbane\n2,Sydney" });
  await app.press("Paste from clipboard"); await app.press("Review import"); await app.press("Import document");
  await app.waitFor("2 rows · 2 columns");
  await app.keyboard.press("mod+f");
  expect(app.shows("id:find-input")).toBe(true);
  expect(app.texts()).not.toContain("0 matches");
  await app.paste("find-input", { text: "Brisbane" });
  await app.waitFor("1 of 2 matches");
  await app.keyboard.press("mod+f");
  expect(app.value("find-input")).toBe("Brisbane");
  expect(app.shows("id:find-input")).toBe(true);
  expect(app.spanStyle("id:active-match").underline).toBe(true);
  expect(app.texts()).toContain("Brisbane Brisbane");
  await app.keyboard.press("enter");
  await app.waitFor("2 of 2 matches");
  expect(app.texts()).toContain("Row 1 · Column 2");
  expect(app.spanStyle("id:active-match").underline).toBe(true);
  await app.keyboard.press("shift+enter");
  await app.waitFor("1 of 2 matches");
  await app.keyboard.press("escape");
  expect(app.shows("id:find-input")).toBe(false);
  expect(app.count("id:search-highlight")).toBe(0);
  await app.keyboard.press("left");
  await app.waitFor("Row 1 · Column 1");
  await app.keyboard.press("mod+f");
  await app.wait(600);
  await app.press("Sydney"); await app.press("Sydney");
  await app.waitFor("Edit city");
  await app.keyboard.press("shift+enter");
  expect(app.value("cell-draft") ?? "").toContain("\n");
  await app.keyboard.press("escape");
  expect(app.shows("id:cell-draft")).toBe(false);
  expect(app.shows("id:find-input")).toBe(true);
  expect(app.texts()).toContain("Sydney");
});

test("find states and the active match stay consistent", async () => {
  const app = await open("./App.tsx", { width: 1280, height: 800, clock: {}, clipboard: "id,city\nb,Brisbane\na,Sydney" });
  await app.press("Paste from clipboard"); await app.press("Review import"); await app.press("Import document");
  await app.waitFor("2 rows · 2 columns");
  await app.press("b");
  await app.press("Find");
  expect(app.texts()).not.toContain("0 matches");
  expect(app.texts()).not.toContain("No matches");
  await app.paste("find-input", { text: "b" });
  expect(app.texts()).toContain("Searching…");
  expect(app.texts()).not.toContain("0 matches");
  const clock = app.clock;
  if (!clock) throw new Error("expected a manual clock");
  await clock.runFor(200);
  await app.waitFor("1 of 3 matches");
  await app.press("Next"); await app.press("Next");
  await app.waitFor("3 of 3 matches");
  expect(app.texts()).toContain("Row 1 · Column 2");
  await app.press("Options");
  await app.press("Case sensitive");
  await app.waitFor("2 of 2 matches");
  expect(app.texts()).toContain("Row 1 · Column 2");
  await app.press("id:search-scope");
  await app.press("city · Column 2");
  await app.waitFor("1 of 1 matches");
  expect(app.texts()).toContain("Row 1 · Column 2");
  expect(app.texts()).toContain("city");
  await app.press("id:search-scope");
  await app.press("All columns");
  await app.waitFor("2 of 2 matches");
  await app.clear("find-input");
  expect(app.texts()).not.toContain("0 matches");
  expect(app.texts()).not.toContain("No matches");
  await app.paste("find-input", { text: "zzzz-absent" });
  await clock.runFor(200);
  await app.waitFor("No matches");
  await app.clear("find-input");
  await app.paste("find-input", { text: "(" });
  await app.press("Regular expression");
  expect(app.texts().some(text => text.toLowerCase().includes("regex") || text.includes("unclosed"))).toBe(true);
  expect(app.texts()).not.toContain("0 matches");
  expect(app.texts()).not.toContain("No matches");
});

test("replace undo and redo keep the same match selected", async () => {
  const app = await open("./App.tsx", { width: 1280, height: 800, clipboard: "id,city\nb,Brisbane Brisbane\na,Sydney\nc,Brisbane" });
  await app.press("Paste from clipboard"); await app.press("Review import"); await app.press("Import document");
  await app.waitFor("3 rows · 2 columns");
  await app.press("Find");
  await app.paste("find-input", { text: "Brisbane" });
  await app.waitFor("1 of 3 matches");
  await app.press("Next"); await app.waitFor("2 of 3 matches");
  await app.press("Replace…");
  await app.paste("replace-input", { text: "City" });
  await app.press("Replace");
  await app.waitFor("1 replacement in 1 cell");
  await app.waitFor("2 of 2 matches");
  expect(app.texts()).toContain("Row 3 · Column 2");
  await app.press("Replace all");
  await app.waitFor("2 replacements in 2 cells");
  await app.waitFor("No matches");
  expect(app.texts()).not.toContain("0 matches");
  await app.press("Undo");
  await app.waitFor("2 of 2 matches");
  expect(app.texts()).toContain("Row 3 · Column 2");
  await app.press("Undo");
  await app.waitFor("3 of 3 matches");
  expect(app.texts()).toContain("Row 3 · Column 2");
  await app.press("Redo");
  await app.waitFor("2 of 2 matches");
  expect(app.texts()).toContain("Row 3 · Column 2");
});

test("find reaches a match past the cell preview", async () => {
  const filler = "x".repeat(520);
  const app = await open("./App.tsx", { width: 1280, height: 800, clipboard: `id,note\n1,${filler}NEEDLE\n2,${filler}NEEDLE` });
  await app.press("Paste from clipboard"); await app.press("Review import"); await app.press("Import document");
  await app.waitFor("2 rows · 2 columns");
  await app.press("Find");
  await app.paste("find-input", { text: "NEEDLE" });
  await app.waitFor("1 of 2 matches");
  await app.waitFor("Row 1 · Column 2");
  expect(app.texts()).toContain("NEEDLE");
  expect(app.shows("id:active-match")).toBe(true);
  expect(app.texts().find(text => text.startsWith("xxx"))?.includes("NEEDLE")).toBe(false);
  await app.press("Next");
  await app.waitFor("2 of 2 matches");
  await app.waitFor("Row 2 · Column 2");
  expect(app.texts()).toContain("NEEDLE");
});

test("a pending search does not step the previous matches", async () => {
  const app = await open("./App.tsx", { width: 390, height: 844, clock: {}, clipboard: "id,city\nb,Brisbane\na,Sydney" });
  await app.press("Paste from clipboard"); await app.press("Review import"); await app.press("Import document");
  await app.waitFor("2 rows · 2 columns");
  await app.press("Find");
  expect(app.shows("id:find-input")).toBe(true);
  expect(app.texts()).toContain("All columns");
  await app.paste("find-input", { text: "b" });
  const clock = app.clock;
  if (!clock) throw new Error("expected a manual clock");
  await clock.runFor(200);
  await app.waitFor("1 of 3 matches");
  await app.press("Next");
  await app.waitFor("2 of 3 matches");
  await app.paste("find-input", { text: "x" });
  expect(app.texts()).toContain("Searching…");
  expect(app.enabled("Next")).toBe(false);
  expect(app.enabled("Previous")).toBe(false);
  await app.keyboard.press("enter");
  expect(app.texts()).toContain("Searching…");
  expect(app.texts()).not.toContain("3 of 3 matches");
  expect(app.texts()).toContain("Row 1 · Column 2");
  await clock.runFor(200);
  await app.waitFor("No matches");
});


test("cancelled Find work cannot publish stale matches after query changes or close", async () => {
  const app = await open("./App.tsx", { width: 390, height: 844, clock: {}, clipboard: "value\n" + "old value\n".repeat(20_000) + "target" });
  await app.press("Paste from clipboard"); await app.press("Review import"); await app.press("Import document");
  await app.waitFor("20,001 rows · 1 column");
  const clock = app.clock;
  if (!clock) throw new Error("expected a manual clock");
  await app.press("Find"); await app.paste("find-input", { text: "old" });
  await app.press("Cancel search");
  await clock.runFor(500);
  await app.waitFor("Search cancelled");
  expect(app.count("id:search-highlight")).toBe(0);
  await app.clear("find-input"); await app.paste("find-input", { text: "target" });
  await clock.runFor(500);
  await app.waitFor("1 of 1 matches");
  await app.clear("find-input"); await app.paste("find-input", { text: "old" });
  await app.press("Close find");
  await clock.runFor(500);
  expect(app.count("id:search-highlight")).toBe(0);
  await app.press("close-document");
  await clock.runFor(500);
  expect(app.shows("id:home-scroll")).toBe(true);
});

test("mobile search chooses a long column name without following the active match", async () => {
  const name = "https://rowtation.example/very-long-column-name";
  const app = await open("./App.tsx", { width: 320, height: 568, clipboard: `${name},other\nneedle,needle` });
  await app.press("Paste from clipboard"); await app.press("Review import"); await app.press("Import document");
  await app.waitFor("1 row · 2 columns");
  await app.press("Find");
  await app.paste("find-input", { text: "needle" });
  await app.waitFor("1 of 2 matches");
  await app.press("id:search-scope");
  await app.press(`text:${name} · Column 1`);
  await app.waitFor("1 of 1 matches");
  await app.press("id:search-scope");
  await app.press("All columns");
  await app.waitFor("1 of 2 matches");
  await app.press("Next");
  await app.waitFor("2 of 2 matches");
  expect(app.texts()).toContain("All columns");
  await app.press("Replace…");
  await app.paste("replace-input", { text: "updated" });
  await app.press("Replace all");
  await app.waitFor("2 replacements in 2 cells");
});

test("scope popup owns Escape Enter and arrow keys before Find", async () => {
  const app = await open("./App.tsx", { width: 1280, height: 800, platform: "macos", clipboard: "name,other\nneedle,needle\nplain,needle" });
  await app.press("Paste from clipboard"); await app.press("Review import"); await app.press("Import document");
  await app.press("Find"); await app.paste("find-input", { text: "needle" });
  await app.waitFor("1 of 3 matches");
  await app.press("id:search-scope");
  expect(app.shows("text:name · Column 1")).toBe(true);
  await app.keyboard.press("escape");
  expect(app.shows("id:find-input")).toBe(true);
  expect(app.shows("text:name · Column 1")).toBe(false);
  await app.keyboard.press("arrowdown");
  expect(app.shows("text:name · Column 1")).toBe(true);
  await app.keyboard.press("arrowdown");
  await app.keyboard.press("enter");
  await app.waitFor("1 of 1 matches");
  expect(app.shows("id:find-input")).toBe(true);
  await app.setViewport(390, 844);
  await app.press("id:search-scope");
  await app.keyboard.press("arrowdown");
  await app.keyboard.press("arrowup");
  await app.keyboard.press("arrowdown");
  await app.keyboard.press("enter");
  await app.waitFor("1 of 2 matches");
  await app.press("Next"); await app.waitFor("2 of 2 matches");
  await app.keyboard.press("escape");
  expect(app.shows("id:find-input")).toBe(false);
});

test("scope trigger distinguishes duplicate and unnamed columns", async () => {
  const app = await open("./App.tsx", { width: 390, height: 844, clipboard: "name,,name,   \nneedle,blank,needle,space" });
  await app.press("Paste from clipboard"); await app.press("Review import"); await app.press("Import document");
  await app.press("Find");
  await app.press("id:search-scope"); await app.press("text:name · Column 3");
  expect(app.texts()).toContain("Column 3 · name");
  await app.press("id:search-scope"); await app.press("text:name · Column 1");
  expect(app.texts()).toContain("Column 1 · name");
  await app.press("id:search-scope"); await app.press("text:Unnamed column · Column 2");
  expect(app.texts()).toContain("Column 2 (unnamed)");
  await app.press("id:search-scope"); await app.press("text:Unnamed column · Column 4");
  expect(app.texts()).toContain("Column 4 (unnamed)");
});
