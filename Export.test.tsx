import { expect, open, test } from "nsh/test";

test("export dialog copies edited JSON and cancellation retains undo and dirty state", async () => {
  const app = await open("./App.tsx", { width: 1280, height: 800, clipboard: "id,city\n001,Brisbane" });
  await app.press("Paste from clipboard"); await app.press("Review import"); await app.press("Import document");
  await app.press("Brisbane"); await app.keyboard.press("f2"); await app.clear("cell-draft");
  await app.paste("cell-draft", {text: "Perth"}); await app.press("Save cell");
  await app.press("Export…"); await app.waitFor("id:export-filename");
  await app.keyboard.press("mod+z"); await app.keyboard.press("f2");
  expect(app.shows("id:cell-draft")).toBe(false);
  await app.press("JSON · JSON"); await app.press("Readable formatting"); await app.press("Copy"); await app.waitFor("Copied export text");
  await app.press("Cancel"); await app.waitFor("1 modified · Unsaved");
  await app.press("Undo"); await app.waitFor("Unmodified");
  expect(app.clipboard()).toBe('[{"id":"001","city":"Perth"}]');
});

test("mobile export contextual options warnings and failed conversion preserve data", async () => {
  const app = await open("./App.tsx", { width: 390, height: 844, clipboard: '[{"n":null,"id":"001"}]' });
  await app.press("Paste from clipboard"); await app.press("Review import"); await app.press("Import document");
  await app.press("Export…"); await app.press("NDJSON · NDJSON");
  expect(app.texts()).not.toContain("Readable formatting");
  expect(app.texts()).not.toContain("Include column headers");
  await app.press("Copy"); await app.waitFor("Copied export text");
  const copied = app.clipboard();
  await app.press("XML · XML"); await app.scroll("dialog-content", 600); await app.press("Accept these mapping changes");
  await app.press("Copy"); await app.waitFor("export-status");
  expect(app.clipboard()).toBe(copied);
  await app.press("Cancel"); await app.waitFor("Unmodified"); await app.waitFor("001");
  await app.press("Export…"); await app.press("CSV · CSV");
  expect(app.texts()).toContain("Accept these mapping changes");
  await app.scroll("dialog-content", 600); await app.press("Accept these mapping changes"); await app.press("Copy"); await app.waitFor("Copied export text");
  await app.keyboard.press("escape"); await app.waitFor("Unmodified");
});

test("nested JSON export places edited missing paths and preserves exact numeric tokens", async () => {
  const input = '[{"items":[{"sku":"001"}],"n":900719925474099312345,"decimal":1.2300},{}]';
  const app = await open("./App.tsx", {width:1280,height:800,clipboard:input});
  await app.press("Paste from clipboard"); await app.press("Review import"); await app.press("Import document");
  await app.press("001"); await app.keyboard.press("arrowdown"); await app.keyboard.press("f2");
  await app.paste("cell-draft", {text:"002"}); await app.press("Save cell");
  await app.press("Export…"); await app.press("Readable formatting"); await app.press("Copy"); await app.waitFor("Copied export text");
  expect(app.clipboard()).toBe('[{"items":[{"sku":"001"}],"n":900719925474099312345,"decimal":1.2300},{"items":[{"sku":"002"}]}]');
  await app.press("Cancel"); await app.press("Undo"); await app.waitFor("Unmodified");
});
