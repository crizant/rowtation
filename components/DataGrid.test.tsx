import { expect, open, test } from "nsh/test";

test("grid pointer cells use native text until search highlights are needed", async () => {
  const app = await open("../App.tsx", { width: 1280, height: 800, platform: "macos", clipboard: "id,city\na,Brisbane Brisbane\nb,Sydney" });
  await app.press("Paste from clipboard"); await app.press("Review import"); await app.press("Import document");
  await app.waitFor("2 rows · 2 columns");
  expect(app.count("id:grid-custom-cell")).toBe(0);
  await app.press("Sydney"); await app.keyboard.press("up");
  await app.waitFor("Row 1 · Column 2");
  await app.press("Find"); await app.paste("find-input", { text: "absent" });
  await app.waitFor("No matches");
  expect(app.count("id:grid-custom-cell")).toBe(0);
  await app.clear("find-input"); await app.paste("find-input", { text: "Brisbane" });
  await app.waitFor("1 of 2 matches");
  expect(app.count("id:search-highlight")).toBe(1);
  await app.waitUntil(() => {
    try { return app.spanStyle("id:active-match").underline; } catch { return false; }
  });
  await app.press("Next"); await app.waitFor("2 of 2 matches");
  expect(app.spanStyle("id:active-match").underline).toBe(true);
  await app.keyboard.press("tab");
  await app.press("Brisbane Brisbane"); await app.keyboard.press("mod+c");
  expect(app.clipboard()).toBe("Brisbane Brisbane");
  await app.press("Close find");
  await app.waitUntil(() => app.count("id:search-highlight") === 0);
  expect(app.count("id:grid-custom-cell")).toBe(0);
  expect(app.count("id:active-match")).toBe(0);
  await app.wait(600); await app.press("Sydney"); await app.press("Sydney");
  await app.waitFor("Edit city");
  expect(app.value("cell-draft")).toBe("Sydney");
});

test("grid pointer highlights survive cache refills and renderer switches", async () => {
  const csv = "id,city\n" + Array.from({ length: 800 }, (_, row) => `${row},${row === 0 || row === 700 ? "café café" : `City ${row}`}`).join("\n");
  const app = await open("../App.tsx", { width: 1280, height: 800, platform: "macos", clipboard: csv });
  await app.press("Paste from clipboard"); await app.press("Review import"); await app.press("Import document");
  await app.press("Find"); await app.paste("find-input", { text: "café" });
  await app.waitFor("1 of 4 matches");
  expect(app.count("id:search-highlight")).toBe(1);
  await app.press("Next"); await app.press("Next");
  await app.waitFor("3 of 4 matches"); await app.waitFor("Row 701 · Column 2");
  expect(app.spanStyle("id:active-match").underline).toBe(true);
  await app.press("Previous"); await app.waitFor("2 of 4 matches");
  await app.waitFor("Row 1 · Column 2");
  expect(app.spanStyle("id:active-match").underline).toBe(true);
  await app.press("Close find");
  expect(app.count("id:grid-custom-cell")).toBe(0);
});

test("grid mobile long-press edits the held cell with and without highlights", async () => {
  const app = await open("../App.tsx", { width: 390, height: 844, platform: "ios", clipboard: "id,city\na,Sydney\nb,Brisbane" });
  await app.press("Paste from clipboard"); await app.press("Review import"); await app.press("Import document");
  await app.press("a");
  await app.longPress("Sydney"); await app.press("Edit cell");
  await app.waitFor("Edit city");
  expect(app.value("cell-draft")).toBe("Sydney");
  await app.press("Cancel");
  await app.press("Find"); await app.paste("find-input", { text: "Brisbane" });
  await app.waitFor("1 of 1 matches");
  await app.longPress("Brisbane"); await app.press("Edit cell");
  await app.waitFor("Edit city");
  expect(app.value("cell-draft")).toBe("Brisbane");
});
