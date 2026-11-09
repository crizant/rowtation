import { expect, open, test } from "nsh/test";

test("JSON review hides delimited settings and unchanged typed edits stay clean", async () => {
  const app = await open("./App.tsx", { width: 1280, height: 800, clipboard: '[{"user":{"name":"Mia"},"amount":900719925474099312345,"enabled":true}]' });
  await app.press("Paste from clipboard"); await app.press("Review import");
  expect(app.texts()).not.toContain("First row contains column headers");
  await app.press("Import document"); await app.waitFor("1 row · 3 columns");
  await app.waitFor("JSON · UTF-8");
  await app.press("900719925474099312345"); await app.keyboard.press("f2");
  expect(app.value("cell-draft")).toBe("900719925474099312345");
  await app.press("Save cell"); await app.waitFor("Unmodified");
  await app.keyboard.press("f2"); await app.clear("cell-draft"); await app.paste("cell-draft", { text: "changed" }); await app.press("Save cell");
  await app.waitFor("1 modified · Unsaved"); await app.press("Undo"); await app.waitFor("Unmodified");
  await app.press("true"); await app.keyboard.press("f2"); expect(app.value("cell-draft")).toBe("true");
  await app.press("Save cell"); await app.waitFor("Unmodified");
  await app.press("Find"); await app.paste("find-input", { text: "900719925474099312345" });
  await app.waitFor("1 of 1 matches");
  expect(app.spanStyle("id:active-match").underline).toBe(true);
});

test("NDJSON line errors can be corrected and format choice stays available", async () => {
  const app = await open("./App.tsx", { width: 390, height: 844, clipboard: '{}\n\n{broken}' });
  await app.press("Paste from clipboard"); await app.press("Review import");
  expect(app.texts().join("\n")).not.toContain("INVALID_JSON");
  await app.press("Import document");
  expect(app.texts().join("\n")).toContain("line 3");
  await app.press("Cancel"); await app.press("Paste from clipboard");
  await app.clear("paste-input"); await app.paste("paste-input", { text: '{"name":"Mia"}\r\n\r\n{"city":"Brisbane"}' });
  await app.press("Review import"); await app.press("Import document");
  await app.waitFor("2 rows · 2 columns"); await app.waitFor("NDJSON · UTF-8");
});

test("XML edits survive cancelled Back and invalid input can be reviewed from home", async () => {
  const app = await open("./App.tsx", { width: 1280, height: 800, clipboard: '<rows><row id="001"><tag>first</tag><tag>second</tag></row></rows>' });
  await app.press("Paste from clipboard"); await app.press("Review import");
  expect(app.texts()).not.toContain("First row contains column headers");
  await app.press("Import document"); await app.waitFor("1 row · 3 columns"); await app.waitFor("XML · UTF-8");
  await app.press("first"); await app.keyboard.press("f2"); await app.clear("cell-draft"); await app.paste("cell-draft", { text: "edited" }); await app.press("Save cell");
  app.onDialog(() => "Keep editing");
  await app.press("close-document"); await app.waitFor("edited");
  await app.press("Undo"); await app.waitFor("Unmodified");
  await app.press("close-document"); await app.press("Paste from clipboard"); await app.clear("paste-input");
  await app.paste("paste-input", { text: '<!DOCTYPE rows><rows/>' }); await app.press("Review import"); await app.press("Import document");
  expect(app.texts().join("\n")).toContain("INVALID_XML");
  await app.press("Cancel"); await app.waitFor("id:home-scroll");
  expect(app.texts()).not.toContain("edited");
});

test("ambiguous paste can be explicitly treated as structured or delimited", async () => {
  const app = await open("./App.tsx", { clipboard: '{"name":"Mia"}' });
  await app.press("Paste from clipboard"); await app.press("Review import");
  await app.press("id:import-format-csv"); expect(app.texts()).toContain("First row contains column headers");
  await app.press("id:import-format-json"); expect(app.texts()).not.toContain("First row contains column headers");
  await app.press("Import document"); await app.waitFor("1 row · 1 column");
});


test("structured review defers parsing and reuses the prepared large candidate", async () => {
  const input = Array.from({length: 513}, (_, i) => `{"field_${i}":${i}}`).join("\n");
  const app = await open("./App.tsx", { width: 1280, height: 800, clipboard: input });
  await app.keyboard.press("ctrl+shift+d");
  await app.press("Paste from clipboard"); await app.press("Review import");
  expect(app.shows("id:import-summary")).toBe(true);
  expect(app.texts().filter(t => t.startsWith("Parse: ")).length).toBe(0);
  await app.press("Import document"); await app.waitFor("This is a large dataset");
  expect(app.texts().join("\n")).toContain("513 rows · 513 columns");
  expect(app.texts().filter(t => t.startsWith("Parse: ")).length).toBe(1);
  await app.press("id:import-format-ndjson"); // Same settings keep the candidate.
  await app.press("Import anyway"); await app.waitFor("513 rows · 513 columns");
  expect(app.texts().filter(t => t.startsWith("Parse: ")).length).toBe(1);
});

test("malformed structured paste remains unparsed until Import", async () => {
  const input = '[{"text":"' + "x".repeat(256 * 1024) + '"}]BROKEN';
  const app = await open("./App.tsx", { width: 390, height: 844, clipboard: input });
  await app.press("Paste from clipboard");
  await app.paste("paste-input", {text: input});
  await app.press("Review import");
  expect(app.shows("id:import-summary")).toBe(true);
  expect(app.texts().join("\n")).not.toContain("INVALID_JSON");
  await app.press("Import document");
  expect(app.texts().join("\n")).toContain("INVALID_JSON");
  await app.press("Cancel"); await app.waitFor("id:home-scroll");
});

test("Back returns home before reviewing and cancelling another structured dataset", async () => {
  const app = await open("./App.tsx", { width: 1280, height: 800, clipboard: '{"name":"Mia"}' });
  await app.press("Paste from clipboard"); await app.press("Review import"); await app.press("Import document");
  await app.waitFor("1 row · 1 column"); await app.press("Mia"); await app.keyboard.press("f2");
  await app.clear("cell-draft"); await app.paste("cell-draft", {text: "edited"}); await app.press("Save cell");
  app.onDialog(() => "Discard changes");
  await app.press("close-document"); await app.waitFor("id:home-scroll");
  await app.press("Paste from clipboard"); await app.clear("paste-input");
  await app.paste("paste-input", {text: Array.from({length: 513}, (_, i) => `{"f${i}":${i}}`).join("\n")});
  await app.press("Review import"); await app.press("Import document"); await app.waitFor("This is a large dataset");
  await app.press("Cancel"); await app.waitFor("id:home-scroll");
  expect(app.texts()).not.toContain("edited");
});
