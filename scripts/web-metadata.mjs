// NSH 0.4 writes the app name into <title> and has no meta description.
// Stamp the packaged index.html so crawlers see both before any script runs.
import { readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

export const htmlTitle = "Rowtation – Free Online CSV & JSON Editor and Converter";
export const metaDescription =
  "Convert JSON to CSV and CSV to JSON in your browser. Edit locally; nothing is uploaded.";

function escapeText(value) {
  return value.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;");
}

function escapeAttr(value) {
  return escapeText(value).replaceAll('"', "&quot;");
}

const titleTag = `<title>${escapeText(htmlTitle)}</title>`;
const descriptionTag = `<meta name="description" content="${escapeAttr(metaDescription)}">`;

/** Returns index.html with the title and description in <head>, before any script. */
export function stampIndexHtml(html) {
  const headEnd = html.search(/<\/head>/i);
  if (headEnd < 0) throw new Error("index.html has no </head>.");
  let head = html.slice(0, headEnd);
  const rest = html.slice(headEnd);
  head = head.replace(/[ \t]*<meta\s+name="description"\s+content="[^"]*"\s*\/?>\n?/gi, "");
  if (!/<title>[\s\S]*?<\/title>/i.test(head)) throw new Error("index.html has no <title> in <head>.");
  head = head.replace(/<title>[\s\S]*?<\/title>/i, `${titleTag}\n${descriptionTag}`);
  const stamped = head + rest;
  const stampedHeadEnd = stamped.search(/<\/head>/i);
  const headHtml = stamped.slice(0, stampedHeadEnd);
  const scriptAt = headHtml.search(/<script\b/i);
  const titleAt = headHtml.indexOf(titleTag);
  const descriptionAt = headHtml.indexOf(descriptionTag);
  if (titleAt < 0 || descriptionAt < 0) throw new Error("SEO metadata was not written into <head>.");
  if (scriptAt >= 0 && (titleAt > scriptAt || descriptionAt > scriptAt)) {
    throw new Error("SEO metadata must appear in the initial HTML, before scripts.");
  }
  return stamped;
}

const invoked = process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href;
if (invoked) {
  const file = process.argv[2];
  if (!file) throw new Error("Usage: node scripts/web-metadata.mjs dist/Rowtation/index.html");
  const html = await readFile(file, "utf8");
  await writeFile(file, stampIndexHtml(html));
}
