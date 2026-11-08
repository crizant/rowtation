// Instrument the real app's registered sessions, without changing their arguments/results.
import "../../App";
import { keymap } from "nsh";
import { AlertDialog } from "nsh/base";
import { DocumentSession } from "rowtation/native";

const dead = new WeakSet<DocumentSession>();
let rows = 0, flags = 0, steps = 0, disposed = 0, released = 0, late = 0, unfinished = 0;
let latest: DocumentSession | undefined;
const metadata = DocumentSession.prototype.metadata;
for (const name of ["rows", "searchHighlights", "searchStep", "startSearch", "cancelSearch", "profileStep", "cancelProfile", "readCell", "metadata"] as const) {
  const original = DocumentSession.prototype[name];
  // Test-only observation of actual bridge calls and use-after-dispose attempts.
  (DocumentSession.prototype as unknown as Record<string, unknown>)[name] = function (this: DocumentSession, ...args: unknown[]) {
    latest = this;
    if (dead.has(this)) late++;
    if (name === "rows") rows++;
    if (name === "searchHighlights") flags++;
    if (name === "searchStep") steps++;
    return Reflect.apply(original, this, args);
  };
}
const dispose = DocumentSession.prototype.dispose;
DocumentSession.prototype.dispose = function () {
  disposed++;
  dead.add(this);
  dispose.call(this);
  try { metadata.call(this); } catch { released++; }
};
// Arm a real unfinished Rust job while the UI's debounce is still pending.
// Manual clocks otherwise drain recursively scheduled zero-delay tasks at once.
keymap.bind("f9", () => {
  latest!.startSearch({ term: "absent" });
  if (!latest!.searchStep().result) unfinished++;
}, { when: "always" });
keymap.bind("f8", () => {
  void AlertDialog.show({ title: `rows=${rows} flags=${flags} steps=${steps} disposed=${disposed} released=${released} late=${late} unfinished=${unfinished}`,
    actions: [{ id: "done", label: "Done" }] });
}, { when: "always" });
