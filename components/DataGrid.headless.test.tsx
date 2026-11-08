import { expect, open, test } from "nsh/test";

// NSH 0.4 cannot emulate an iOS platform in its web test host.
// This checks the touch handler through headless dispatch; physical browser
// long-press is a separate, documented NSH hit-layer limitation.
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
