import assert from "node:assert/strict";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawn } from "node:child_process";
import test from "node:test";
import {
  assertMetadataBeforeScripts,
  escapeHtmlAttribute,
  escapeHtmlText,
  htmlTitle,
  metaDescription,
  stampIndexHtml,
} from "./web-metadata.mjs";

// Same shape as NSH's packaged index.html: a head script, then a module that
// would set the title only after JavaScript runs.
const packaged = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Rowtation</title>
<meta name="viewport" content="width=device-width, initial-scale=1">
<script>const fetches = new Map();</script>
</head>
<body>
<script type="module">document.title = "too late";</script>
</body>
</html>
`;

test("inserts the title and description into the head before any script", () => {
  const html = stampIndexHtml(packaged);
  const head = html.slice(0, html.search(/<\/head>/i));
  assert.equal(head.includes(`<title>${escapeHtmlText(htmlTitle)}</title>`), true);
  assert.equal(head.includes(`<meta name="description" content="${escapeHtmlAttribute(metaDescription)}">`), true);
  assert.ok(head.indexOf("<title>") < head.indexOf("<script"));
  assert.ok(head.indexOf('name="description"') < head.indexOf("<script"));
  assert.equal(head.includes("document.title"), false);
  assertMetadataBeforeScripts(html);
});

test("stamping again leaves the document unchanged", () => {
  const once = stampIndexHtml(packaged);
  assert.equal(stampIndexHtml(once), once);
  const withOld = stampIndexHtml(packaged.replace("</title>", '</title>\n<meta name="description" content="old &amp; stale">'));
  assert.equal(withOld.match(/name="description"/g).length, 1);
  assert.equal(withOld.includes("stale"), false);
  assert.equal(stampIndexHtml(withOld), withOld);
});

test("escapes HTML in the title and attribute text", () => {
  assert.equal(escapeHtmlText(`&<>`), "&amp;&lt;&gt;");
  assert.equal(escapeHtmlAttribute(`"&`), "&quot;&amp;");
  const title = stampIndexHtml(packaged).match(/<title>([^<]*)<\/title>/)[1];
  assert.equal(title, escapeHtmlText(htmlTitle));
  assert.equal(title.includes("CSV & JSON"), false);
  assert.equal(title.includes("CSV &amp; JSON"), true);
});

test("rejects a document missing head or title", () => {
  assert.throws(() => stampIndexHtml("<html><body><title>Rowtation</title></body></html>"), /no <\/head>/);
  assert.throws(() => stampIndexHtml("<html><head></head></html>"), /no <title>/);
  assert.throws(
    () => stampIndexHtml("<html><head><script>document.title='Rowtation'</script></head></html>"),
    /no <title>/,
  );
  assert.throws(() => assertMetadataBeforeScripts(packaged), /not written into <head>/);
});

test("a stamped package file keeps the metadata before scripts", async () => {
  const dir = await mkdtemp(join(tmpdir(), "rowtation-seo-"));
  const file = join(dir, "index.html");
  try {
    await writeFile(file, packaged);
    await run(["scripts/web-metadata.mjs", file]);
    const html = await readFile(file, "utf8");
    assertMetadataBeforeScripts(html);
    await run(["scripts/web-metadata.mjs", "--check", file]);
    await writeFile(file, packaged);
    await assert.rejects(run(["scripts/web-metadata.mjs", "--check", file]));
  } finally {
    await rm(dir, { recursive: true });
  }
});

function run(args) {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, args, { stdio: "ignore" });
    child.on("error", reject);
    child.on("exit", (code) => (code === 0 ? resolve() : reject(new Error("exit " + code))));
  });
}
