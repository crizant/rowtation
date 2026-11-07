import { expect, open, test } from "nsh/test";

test("production app mounts", async () => {
  const app = await open("./App.tsx");
  expect(app.texts()).toContain("Rowtation");
});

test("app runtime creates, queries, serializes and disposes the Rust document", async () => {
  const app = await open("./tests/fixtures/DocumentSmoke.tsx");
  expect(app.texts()).toContain("1 row, 1 column: 00123");
  expect(app.texts()).toContain("Bridge checks passed");
  await app.press("Close document");
  expect(app.texts()).toContain("Disposed");
});
