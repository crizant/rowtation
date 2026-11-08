#!/bin/sh
# Set ROWTATION_BROWSER when Chromium is outside NSH's default discovery paths.
set -eu
# NSH 0.4.0 reads a real browser host only from nsh.json. Restore the exact
# configuration on exit (including test failures); no test host is packaged.
rowtation_config_backup=$(mktemp)
cp nsh.json "$rowtation_config_backup"
trap 'cp "$rowtation_config_backup" nsh.json; rm -f "$rowtation_config_backup"' EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM
python3 - <<'CONFIG'
import json
from pathlib import Path
path = Path("nsh.json")
config = json.loads(path.read_text())
config["web"] = {"host": "tests/browser-host.js"}
path.write_text(json.dumps(config, indent=2) + "\n")
CONFIG
if [ -n "${ROWTATION_BROWSER:-}" ]; then
  set -- --browser "$ROWTATION_BROWSER"
fi
nsh test App.tsx --target web "$@" <<'STEPS'
await app.setViewport(1280, 800);
// Structured formats use the same real File picker/drop bridge.
await app.keyboard.press("F11");
await app.press("Choose file…");
await app.waitFor("Import “picked.json”");
if (app.texts().includes("First row contains column headers")) throw new Error("JSON exposed CSV settings");
await app.press("Import document");
await app.waitFor("1 row · 2 columns");
await app.waitFor("JSON · UTF-8");
if (!app.texts().includes("900719925474099312345")) throw new Error("Picker lost numeric precision");
await app.keyboard.press("F12");
await app.waitFor("Import “dropped.xml”");
await app.press("Import document");
await app.waitFor("XML · UTF-8");
if (!app.texts().includes("001") || !app.texts().includes("Hana")) throw new Error("XML drop lost fields");
await app.keyboard.press("F5");
await app.waitFor("Import “dropped.ndjson”");
await app.press("Import document");
await app.waitFor("NDJSON · UTF-8");
await app.waitFor("2 rows · 2 columns");
await app.press("close-document");
await app.press("Choose file…");
await app.waitFor("Import “picked.csv”");
await app.press("Import document");
await app.waitFor("2 rows · 7 columns");
if (!app.texts().includes("00123")) throw new Error("Picker lost literal text");
await app.keyboard.press("F9");
await app.waitFor("Drop one data file at a time.");
if (!app.texts().includes("picked.csv")) throw new Error("Multiple drop replaced document");
await app.keyboard.press("F7");
await app.waitFor("unsupported.xlsx: Choose a .csv, .tsv, .json, .ndjson, .jsonl or .xml file.");
await app.keyboard.press("F10");
await app.waitFor("Import “broken.csv”");
await app.press("Import document");
await app.waitFor("Correct the input or choose another format or file. Your current document is unchanged.");
await app.press("Cancel");
if (!app.texts().includes("picked.csv")) throw new Error("Invalid drop replaced document");
await app.keyboard.press("F8");
await app.waitFor("Import “dropped.tsv”");
await app.press("Import document");
await app.waitFor("1 row · 2 columns");
if (!app.texts().includes("Hana") || app.texts().includes("00123")) throw new Error("Drop did not replace document");
await app.press("close-document");
await app.waitFor("Rowtation");
await app.press("Choose file…");
await app.waitFor("Import “picked.csv”");
await app.press("Import document");
await app.waitFor("2 rows · 7 columns");

// A rejected file must remove the previous review draft, never the open document.
for (const [key, message] of [
  ["F7", "unsupported.xlsx: Choose a .csv, .tsv, .json, .ndjson, .jsonl or .xml file."],
  ["F4", "invalid-utf8.csv: INVALID_UTF8: Save the file as UTF-8 and try again."],
]) {
  await app.press("More");
  await app.press("Import…");
  await app.press("Choose another file…");
  await app.waitFor("Import “picked.csv”");
  await app.keyboard.press(key);
  await app.waitFor(message);
  if (app.shows("app-dialog") || !app.texts().includes("00123")) throw new Error("Rejected file retained an importable draft or replaced the open document");
}

// Release controlled promises, then let NSH deliver their queued callbacks.
async function releaseRead() {
  await app.keyboard.press("F3");
  await app.wait(50);
}
await app.press("More");
await app.press("Import…");
await app.keyboard.press("F2");
await app.press("Cancel");
await releaseRead();
if (app.shows("app-dialog") || !app.texts().includes("00123")) throw new Error("Cancelled read reopened the dialog or changed the document");

await app.keyboard.press("F2");
await app.keyboard.press("F8");
await app.waitFor("Import “dropped.tsv”");
await releaseRead();
if (app.texts().includes("Import “delayed.csv”")) throw new Error("Older read replaced the newer file draft");
await app.press("Import document");
await app.waitFor("1 row · 2 columns");

await app.press("close-document");
await app.keyboard.press("F2");
await app.press("Paste from clipboard");
await app.clear("paste-input");
await app.paste("paste-input", { text: "name\nPaste replacement" });
await releaseRead();
if (app.value("paste-input") !== "name\nPaste replacement") throw new Error("Older file read replaced the newer paste workflow");
await app.press("Review import");
await app.press("Import document");
await app.waitFor("Paste replacement");

// Large files are confirmed before the browser reads them, and a read can be cancelled.
await app.keyboard.press("F6");
await app.waitFor("This file is 8.6 MB. Rowtation has not read it yet.");
if (!app.texts().includes("Read file")) throw new Error("Large file was not held for confirmation");
await app.press("Cancel");
if (app.shows("app-dialog") || !app.texts().includes("Paste replacement")) throw new Error("Declined large file changed the document");
await app.keyboard.press("F6");
await app.press("Read file");
await app.waitFor("Reading…");
await app.press("Cancel");
await releaseRead();
if (app.shows("app-dialog") || !app.texts().includes("Paste replacement")) throw new Error("Cancelled large read reopened the dialog or changed the document");
await app.keyboard.press("F6");
await app.press("Read file");
await releaseRead();
await app.waitFor("8.6 MB · 4,500,000 rows · 1 column · longest field 2 bytes");
if (app.texts().includes("This is a large dataset")) throw new Error("Confirmed file was warned about twice");
await app.press("Import document");
await app.waitFor("4,500,000 rows · 1 column", { timeout: 30000 });

STEPS
