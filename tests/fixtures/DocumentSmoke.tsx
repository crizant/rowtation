// Test-only entry: bridge calls run inside the app (QuickJS headless, browser
// JavaScript + Rust/WASM on web). The web test driver's VM has no app-native API.
import { cleanup, mount, signal, Text, View } from "nsh";
import { DocumentSession, ImportSource } from "rowtation/native";
import { Button } from "../../components/Button";

function throwsMessage(action: () => unknown, message: string): boolean {
  try { action(); } catch (error) {
    return error instanceof Error && error.message.includes(message);
  }
  return false;
}

// Past the former 1 MiB value cap, with a multibyte character straddling the preview cut.
function verifyLongCell() {
  const text = "a" + "雪".repeat(350_000);
  const source = ImportSource.fromText(text, "long.csv");
  const session = DocumentSession.fromSource(source, { headers: false });
  source.dispose();
  try {
    const whole = session.rows({ offset: 0, limit: 1 }).rows[0].cells[0];
    const preview = session.rows({ offset: 0, limit: 1, previewBytes: 512, columnOffset: 0, columnLimit: 1 }).rows[0].cells[0];
    const full = session.readCell(1, session.columns()[0].id);
    if (whole.text !== text || whole.truncated || preview.text !== "a" + "雪".repeat(170) || !preview.truncated ||
      preview.byteLength !== 1 + 3 * 350_000 || full.text !== text) {
      throw new Error("Long cell preview/readCell contract failed");
    }
  } finally {
    session.dispose();
  }
}

function DocumentSmoke() {
  verifyLongCell();
  const fixture = new Uint8Array([105, 100, 10, 48, 48, 49, 50, 51, 10]);
  const session = new DocumentSession(fixture, {});
  let live = true;
  cleanup(() => { if (live) session.dispose(); });
  const result = signal("Bridge checks passed");
  const metadata = session.metadata();
  const columns = session.columns();
  const window = session.rows({ offset: 0, limit: 1 });
  const cell = session.readCell(window.rows[0].id, columns[0].id);
  const output = session.serialize({});
  if (metadata.rowCount !== 1 || metadata.columnCount !== 1 || metadata.dirty ||
    metadata.revision !== 0 || window.documentId !== metadata.id ||
    cell.byteLength !== 5 || window.rows[0].cells[0].text !== "00123" ||
    columns[0].name !== "id" || cell.kind !== "text" || cell.text !== "00123" ||
    output.length !== fixture.length || output.some((byte, i) => byte !== fixture[i]) ||
    !throwsMessage(() => session.rows({ offset: 0, limit: 257 }), "INVALID_WINDOW") ||
    !throwsMessage(() => new DocumentSession(new Uint8Array([255]), {}), "INVALID_UTF8") ||
    session.metadata().id !== metadata.id) {
    throw new Error("Document bridge contract failed");
  }
  return (
    <View class="flex-1 bg-background items-center justify-center gap-4">
      <Text class="text-foreground">{`${metadata.rowCount} row, ${metadata.columnCount} column: ${cell.text}`}</Text>
      <Text class="text-foreground">{result()}</Text>
      <Button label="Close document" onPress={() => {
        session.dispose();
        live = false;
        result.set(throwsMessage(() => session.metadata(), "disposed") ? "Disposed" : "Disposal failed");
      }} />
    </View>
  );
}
mount(DocumentSmoke);
