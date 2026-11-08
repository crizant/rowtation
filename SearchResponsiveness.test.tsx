import { expect, open, test } from "nsh/test";

test("large Rust search yields to event-loop tasks before completion and can restart", async () => {
  const app = await open("./tests/fixtures/SearchResponsiveness.tsx");
  await app.press("Search");
  await app.waitFor("Yielded between slices", { timeout: 30000 });
  expect(app.texts().some(text => text.includes("longest"))).toBe(true);
  await app.press("Restart");
  await app.waitFor("Restart correct", { timeout: 30000 });
});
