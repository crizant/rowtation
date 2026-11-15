// Stamp the packaged index.html so crawlers see metadata before any script runs.
import { readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

export const htmlTitle = "Rowtation – Free Online CSV & JSON Editor and Converter";
export const metaDescription =
  "Free online CSV and JSON editor and converter. Convert JSON to CSV, CSV to JSON, and more. View, search, and edit data privately in your browser.";
export const googleSiteVerification = "Unz_MM3MCOpCUBlZn6fNO7N-gtrnpoTqEmnC0iDkidE";

export function escapeHtmlText(value) {
  return value.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;");
}

export function escapeHtmlAttribute(value) {
  return escapeHtmlText(value).replaceAll('"', "&quot;");
}

const titleTag = `<title>${escapeHtmlText(htmlTitle)}</title>`;
const descriptionTag = `<meta name="description" content="${escapeHtmlAttribute(metaDescription)}">`;
const verificationTag = `<meta name="google-site-verification" content="${escapeHtmlAttribute(googleSiteVerification)}">`;

/** The metadata is in <head> and precedes every script. */
export function assertMetadataBeforeScripts(html) {
  const headEnd = html.search(/<\/head>/i);
  if (headEnd < 0) throw new Error("index.html has no </head>.");
  const head = html.slice(0, headEnd);
  const scriptAt = head.search(/<script\b/i);
  const titleAt = head.indexOf(titleTag);
  const descriptionAt = head.indexOf(descriptionTag);
  const verificationAt = head.indexOf(verificationTag);
  if (titleAt < 0 || descriptionAt < 0 || verificationAt < 0) throw new Error("SEO metadata was not written into <head>.");
  if (scriptAt >= 0 && (titleAt > scriptAt || descriptionAt > scriptAt || verificationAt > scriptAt)) {
    throw new Error("SEO metadata must appear in the initial HTML, before scripts.");
  }
}

/** Returns index.html with metadata in <head>, before any script. */
export function stampIndexHtml(html) {
  const headEnd = html.search(/<\/head>/i);
  if (headEnd < 0) throw new Error("index.html has no </head>.");
  let head = html.slice(0, headEnd);
  const rest = html.slice(headEnd);
  head = head.replace(/[ \t]*<meta\s+name="description"\s+content="[^"]*"\s*\/?>\n?/gi, "");
  head = head.replace(/[ \t]*<meta\s+name="google-site-verification"\s+content="[^"]*"\s*\/?>\n?/gi, "");
  if (!/<title>[\s\S]*?<\/title>/i.test(head)) throw new Error("index.html has no <title> in <head>.");
  head = head.replace(/<title>[\s\S]*?<\/title>/i, `${titleTag}\n${descriptionTag}\n${verificationTag}`);
  const stamped = head + rest;
  assertMetadataBeforeScripts(stamped);
  return stamped;
}

const invoked = process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href;
if (invoked) {
  const check = process.argv[2] === "--check";
  const file = check ? process.argv[3] : process.argv[2];
  if (!file) throw new Error("Usage: node scripts/web-metadata.mjs [--check] dist/Rowtation/index.html");
  const html = await readFile(file, "utf8");
  if (check) assertMetadataBeforeScripts(html);
  else await writeFile(file, stampIndexHtml(html));
}
