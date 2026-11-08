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
  expect(app.texts()).toContain("Unique values");
  expect(app.texts()).toContain("COLUMN · CITY");
  await app.press("close-document");
  expect(app.texts()).toContain("Rowtation");
  expect(app.texts()).not.toContain("00123");
});

test("failed import preserves the document and successful import replaces it", async () => {
  const app = await open("./App.tsx", { clipboard: "name\nOriginal" });
  await app.press("Paste from clipboard");
  await app.press("Review import");
  await app.press("Import document");
  await app.waitFor("1 row · 1 column");
  await app.press("Import…");
  await app.paste("paste-input", { text: '\n"unclosed' });
  await app.press("Review import");
  await app.press("Import document");
  await app.waitFor("text:Check the separator and quoted fields, or choose another file. Your current document is unchanged.");
  await app.press("Cancel");
  expect(app.texts()).toContain("Original");
  await app.press("Import…");
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
  expect(app.texts()).toContain("COLUMN · COLUMN 1");
  await app.press("Done");
  await app.setViewport(1280, 800);
  expect(app.texts()).toContain("Unique values");
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
  expect(app.texts()).toContain("Rowtation");
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
  await app.waitFor("Showing 32,768 of 40,003 characters. Copy value copies all of it.");
  await app.press("Show more");
  await app.waitFor(long);
  await app.press("copy-full-value");
  await app.waitFor("Copied");
  expect(app.clipboard()).toBe(long);
});

test("diagnostics are off until requested and stay in the tab", async () => {
  const app = await open("./App.tsx", { width: 1280, height: 800, clipboard: "id\n1\n2" });
  expect(app.texts()).not.toContain("Diagnostics · local only");
  await app.keyboard.press("ctrl+shift+d");
  await app.waitFor("Diagnostics · local only");
  await app.press("Paste from clipboard");
  await app.press("Review import");
  await app.press("Import document");
  await app.waitFor("2 rows · 1 column");
  const log = app.texts().join("\n");
  expect(log).toContain("Inspect: ");
  expect(log).toContain("Parse: ");
  expect(log).toContain("Row window: ");
  await app.keyboard.press("ctrl+shift+d");
  expect(app.texts()).not.toContain("Diagnostics · local only");
});


test("long pasted text stays inside the field and import actions remain reachable in short dialogs", async () => {
  const app = await open("./App.tsx", { width: 1066, height: 688 });
  await app.press("Paste from clipboard");
  await app.paste("paste-input", { text: "id,city\n" + Array.from({ length: 100 }, (_, i) => `${i + 1},Brisbane`).join("\n") });
  // The outer dialog has no scroll range when this content fits.
  expect(app.texts()).toContain("Paste your data");
  await app.setViewport(390, 360);
  expect(app.texts()).toContain("Paste your data");
  await app.press("Review import");
  expect(app.texts()).toContain("Import “Pasted data”");
  await app.press("Import document");
  await app.waitFor("100 rows · 2 columns");
});

test("search navigation and scoped replacement remain correct after sort", async () => {
  const app = await open("./App.tsx", { width: 1280, height: 800, clipboard: "id,city\nb,Brisbane Brisbane\na,Sydney\nc,Brisbane" });
  await app.press("Paste from clipboard"); await app.press("Review import"); await app.press("Import document");
  await app.waitFor("3 rows · 2 columns");
  await app.press("Sydney"); await app.press("Find & replace");
  await app.paste("find-input", { text: "Brisbane" }); await app.waitFor("1 of 3 matches");
  expect(app.count("id:search-highlight") > 0).toBe(true);
  await app.press("Next"); await app.waitFor("2 of 3 matches");
  await app.press("Next"); await app.waitFor("Row 3 · Column 2");
  await app.press("Previous"); await app.waitFor("2 of 3 matches");
  await app.paste("replace-input", { text: "City" }); await app.press("Replace");
  await app.waitFor("1 replacement in 1 cell");
  await app.press("Replace all"); await app.waitFor("2 replacements in 2 cells");
  await app.press("Undo"); await app.waitFor("1 of 2 matches");
  await app.press("Undo"); await app.waitFor("1 of 3 matches");
  await app.press("Close find");
  await app.waitUntil(() => app.count("id:search-highlight") === 0);
  await app.press("a"); await app.press("Sort ↓");
  await app.press("c"); await app.press("Edit cell…");
  await app.clear("cell-draft"); await app.paste("cell-draft", { text: "z" }); await app.press("Save cell");
  await app.waitFor("1 modified · Unsaved");
  await app.press("Reset order"); await app.press("z"); await app.waitFor("Row 3 · Column 1");
  await app.press("Undo"); await app.waitFor("Unmodified");
});

test("mobile touch editing supports commit cancel and undo", async () => {
  const app = await open("./App.tsx", { width: 390, height: 844, clipboard: "name,city\nMia,Brisbane" });
  await app.press("Paste from clipboard"); await app.press("Review import"); await app.press("Import document");
  await app.waitFor("1 row · 2 columns"); await app.press("Mia"); await app.press("Edit cell…");
  await app.clear("cell-draft"); await app.paste("cell-draft", { text: "Ann" }); await app.press("Save cell");
  await app.waitFor("Ann"); await app.press("Edit cell…");
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
});
