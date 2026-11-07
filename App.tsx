import { cleanup, Clipboard, mount, Scroll, signal, Text, View, windowSize } from "nsh";
import { Checkbox, Toolbar } from "nsh/base";
import { BrowserImport, BrowserStyles, DocumentSession, ImportDetector, type ColumnMetadata, type ColumnProfile, type DocumentMetadata, type LocalImport } from "rowtation/native";
import { Badge } from "./components/Badge";
import { Button } from "./components/Button";
import { DataGrid, type CellSelection } from "./components/DataGrid";
import { Dialog } from "./components/Dialog";
import { Input } from "./components/Input";
import { Inspector } from "./components/Inspector";

interface OpenDocument {
  session: DocumentSession;
  metadata: DocumentMetadata;
  columns: ColumnMetadata[];
  name: string;
  dispose: () => void;
}

function DocumentView(props: { document: OpenDocument; onClose: () => void; onImport: () => void }) {
  cleanup(() => props.document.dispose());
  const selection = signal<CellSelection | null>(null);
  const column = signal<ColumnMetadata | null>(null);
  const profile = signal<ColumnProfile | null>(null);
  const info = signal(false);
  function select(cell: CellSelection | null, columnId: number | null) {
    selection.set(cell);
    if (column()?.id === columnId) return;
    column.set(props.document.columns.find(item => item.id === columnId) ?? null);
    profile.set(columnId == null ? null : props.document.session.profile(columnId));
  }
  return <View class="flex-1 min-h-0 min-w-0 bg-muted/20">
    <Toolbar accessibilityLabel="Document toolbar" class="min-h-16 shrink-0 flex-row items-center gap-3 border-b border-border px-4 py-2 md:px-5">
      <Button label="Close document" quiet icon="caret-left" onPress={() => props.onClose()} testID="close-document" />
      <View class="flex-1 min-w-0 flex-row items-center gap-3">
        <Text class="line-clamp-1 shrink text-base font-semibold text-foreground">{props.document.name}</Text>
        <Badge label={props.document.metadata.delimiter === "\t" ? "TSV" : "CSV"} />
      </View>
      <Button label="Import" quiet onPress={() => props.onImport()} />
    </Toolbar>
    <View class="flex-1 min-h-0 min-w-0 flex-row">
      <View class="flex-1 min-w-0 min-h-0">
        <DataGrid session={props.document.session} metadata={props.document.metadata} columns={props.document.columns} onSelect={select} />
        <View class="shrink-0 min-h-11 flex-row items-center justify-between gap-3 border-t border-border px-4 py-2">
          <Text class="text-xs text-muted-foreground">{props.document.metadata.rowCount.toLocaleString()} {props.document.metadata.rowCount === 1 ? "row" : "rows"} · {props.document.metadata.columnCount} {props.document.metadata.columnCount === 1 ? "column" : "columns"}</Text>
          <If when={windowSize().width >= 900} else={<Button label="Column info" quiet onPress={() => info.set(true)} />}>
            <Text class="text-xs text-muted-foreground">{selection() ? `Row ${selection()!.rowIndex + 1} · Column ${selection()!.columnIndex + 1}` : "Read only · Select a cell to inspect"}</Text>
          </If>
        </View>
      </View>
      <If when={windowSize().width >= 900}>
        <View testID="inspector-panel" class="w-72 shrink-0 min-h-0 m-4 ml-3 overflow-hidden rounded-3xl border border-border bg-background shadow-sm">
          <Scroll testID="inspector-scroll" class="flex-1 min-h-0">
            <Inspector column={column()} profile={profile()} metadata={props.document.metadata} selection={selection()} />
          </Scroll>
        </View>
      </If>
    </View>
    <If when={info() && windowSize().width < 900}>
      <Dialog title="Column information" onClose={() => info.set(false)}>
        <Scroll style={{ maxHeight: windowSize().height * 0.6 }} class="min-h-0"><Inspector column={column()} profile={profile()} metadata={props.document.metadata} selection={selection()} /></Scroll>
        <Button label="Done" onPress={() => info.set(false)} />
      </Dialog>
    </If>
  </View>;
}

export function App() {
  // NSH 0.4 draws a desktop bar for horizontal scroll views even when their
  // indicator is false. DataTable already supplies a bottom viewport bar.
  // Scope this presentation workaround to that table's disabled inner indicator.
  cleanup(BrowserStyles.install('[data-testid="data-grid"] .nsh-noindicator > .nsh-bar { display: none; }'));

  const active = signal<OpenDocument | null>(null);
  const mode = signal<"paste" | "review" | null>(null);
  const draft = signal<LocalImport | null>(null);
  const paste = signal("");
  const delimiter = signal(",");
  const headers = signal(true);
  const ambiguous = signal(false);
  const error = signal("");
  const busy = signal(false);
  const dragging = signal(false);
  let alive = true;
  let request = 0;
  cleanup(() => { alive = false; request++; BrowserImport.cancelPending(); active()?.dispose(); });

  function resetImport() {
    request++;
    BrowserImport.cancelPending();
    draft.set(null); busy.set(false); error.set("");
  }
  function dismiss() { resetImport(); mode.set(null); paste.set(""); }
  function fail(value: unknown) { error.set(value instanceof Error ? value.message : String(value)); }
  function review(file: LocalImport) {
    // Only a manual paste may retain its own editable text after failed detection.
    const pasted = mode() === "paste" && file.requestId == null;
    resetImport();
    mode.set(pasted ? "paste" : null);
    try {
      if (file.error) throw new Error(file.error);
      const result = ImportDetector.inspect(file.bytes, file.name);
      draft.set(file); delimiter.set(result.delimiter); ambiguous.set(result.ambiguous); headers.set(true); mode.set("review");
    } catch (cause) {
      const message = cause instanceof Error ? cause.message : String(cause);
      error.set(file.name ? `${file.name}: ${message}` : message);
    }
  }
  cleanup(BrowserImport.onFile(file => {
    if (alive && file.requestId != null && BrowserImport.isCurrent(file.requestId)) review(file);
  }, active => { if (alive) dragging.set(active); }));

  function pick() {
    dismiss();
    try { BrowserImport.pick(); } catch (cause) { fail(cause); }
  }
  async function openPaste() {
    dismiss();
    const id = request;
    mode.set("paste");
    try {
      const text = await Clipboard.readText();
      if (alive && id === request && !paste()) paste.set(text);
    } catch { /* The paste field remains available without clipboard permission. */ }
  }
  async function importDocument() {
    const file = draft();
    if (mode() !== "review" || !file || busy()) return;
    BrowserImport.cancelPending();
    const id = ++request;
    const options = { delimiter: delimiter(), headers: headers() };
    busy.set(true); error.set("");
    // Let the loading state paint before the synchronous, bounded Rust parser runs.
    await new Promise(resolve => setTimeout(resolve, 30));
    if (!alive || id !== request) return;
    let candidate: DocumentSession | null = null;
    try {
      candidate = new DocumentSession(file.bytes, options);
      const session = candidate;
      const metadata = session.metadata();
      const columns = session.columns();
      let live = true;
      const document: OpenDocument = { session, metadata, columns, name: file.name,
        dispose: () => { if (live) { live = false; session.dispose(); } } };
      active.set(document);
      draft.set(null); paste.set(""); mode.set(null);
    } catch (cause) { candidate?.dispose(); fail(cause); }
    finally { busy.set(false); }
  }

  return <View class="flex-1 min-h-0 min-w-0 bg-muted/25 pt-[env(safe-area-inset-top)] pr-[env(safe-area-inset-right)] pb-[env(safe-area-inset-bottom)] pl-[env(safe-area-inset-left)]">
    <Each of={active() ? [active()!] : []} key={document => document.metadata.id}>
      {document => <DocumentView document={document()} onClose={() => { dismiss(); active.set(null); }} onImport={openPaste} />}
    </Each>
    <If when={!active()}>
      <Scroll class="flex-1 min-h-0" contentStyle={{ flexGrow: 1 }}>
        <View class="flex-1 items-center justify-center px-5 py-12 md:py-20">
          <View class="w-full max-w-[800px] gap-8 md:gap-10">
            <View class="items-start gap-4 md:items-center">
              <View class="h-20 w-20 items-center justify-center rounded-3xl border border-border bg-background shadow-lg">
                <Text class="text-5xl font-medium text-foreground">⟳</Text>
              </View>
              <View class="gap-2 md:items-center">
                <Text class="text-4xl font-semibold tracking-tight text-foreground">Rowtation</Text>
                <Text class="text-base text-muted-foreground">Import and inspect CSV and TSV.</Text>
              </View>
            </View>
            <View testID="import-area" classList={{ "ring-2 ring-ring": dragging() }} class="min-h-72 items-center justify-center gap-6 rounded-3xl border border-border bg-background p-8 shadow-sm md:min-h-80">
              <Text class="text-4xl text-foreground">↥</Text>
              <View class="items-center gap-2">
                <Text class="text-xl font-semibold text-foreground">{windowSize().width < 640 ? "Import your data" : "Drop a file here"}</Text>
                <Text class="text-sm text-center text-muted-foreground">{windowSize().width < 640 ? "Choose a file or paste text to get started." : "Or choose a local file, or paste from your clipboard."}</Text>
              </View>
              <View class="flex-row gap-3"><Badge label="CSV" /><Badge label="TSV" /></View>
            </View>
            <View class="gap-3 md:flex-row md:justify-center">
              <Button label="Paste from clipboard" quiet onPress={openPaste} />
              <Button label="Choose file…" onPress={pick} />
            </View>
            <Text class="text-xs text-center text-muted-foreground">Your data stays in this browser. Nothing is uploaded. UTF-8 · Up to 8 MiB.</Text>
          </View>
        </View>
      </Scroll>
    </If>
    <If when={error() && !mode()}><View role="alert" class="border-t border-border bg-background p-4"><Text class="text-sm text-destructive">{error()}</Text></View></If>
    <If when={mode()}>
      <Dialog title={mode() === "paste" ? "Paste your data" : "Import document"} onClose={dismiss}>
        <If when={mode() === "paste"}>
          <Text class="text-sm text-muted-foreground">Paste CSV or TSV text below. You can review the format before opening it.</Text>
          <Input label="CSV or TSV text" testID="paste-input" value={paste()} onChange={value => paste.set(value)} multiline placeholder={"name,city\nMia,Brisbane"} />
        </If>
        <If when={mode() === "review"}>
          <View class="gap-2"><Text class="line-clamp-1 text-base font-medium text-foreground">{draft()?.name}</Text>
            <Text class="text-sm text-muted-foreground">{ambiguous() ? "Confirm the separator below; detection is uncertain." : "Format detected. Review the options before importing."}</Text></View>
          <View class="flex-row gap-3">
            <Button label={delimiter() === "," ? "✓ CSV · Comma" : "CSV · Comma"} quiet={delimiter() !== ","} disabled={busy()} onPress={() => delimiter.set(",")} />
            <Button label={delimiter() === "\t" ? "✓ TSV · Tab" : "TSV · Tab"} quiet={delimiter() !== "\t"} disabled={busy()} onPress={() => delimiter.set("\t")} />
          </View>
          <Checkbox checked={headers()} disabled={busy()} onChange={event => headers.set(event.checked)}
            class="cursor-pointer disabled:cursor-not-allowed min-h-11 flex-row items-center gap-3 focus:ring-2 focus:ring-ring"
            indicatorClass="h-5 w-5 items-center justify-center rounded border border-border bg-background checked:bg-primary"
            indicator={<Text class="text-sm text-primary-foreground">{headers() ? "✓" : ""}</Text>}>
            <Text class="text-sm text-foreground">First row contains column headers</Text>
          </Checkbox>
          <Text class="text-xs text-muted-foreground">Values are preserved as text, including leading zeros and formula-like content.</Text>
        </If>
        <If when={error()}><View role="alert" class="gap-2"><Text class="text-sm text-destructive">{error()}</Text>
          <Text class="text-xs text-muted-foreground">Check the separator and quoted fields, or choose another file. Your current document is unchanged.</Text></View></If>
        <View class="flex-row justify-end gap-3">
          <Button label="Cancel" quiet onPress={dismiss} />
          <If when={mode() === "paste"} else={<Button label={busy() ? "Importing…" : "Import document"} disabled={busy() || !draft()} onPress={importDocument} />}>
            <Button label="Review import" disabled={!paste()} onPress={() => review({ name: "Pasted data", bytes: new TextEncoder().encode(paste()) })} />
          </If>
        </View>
        <If when={mode() === "paste" || (error() && !busy())}><Button label="Choose another file…" quiet onPress={pick} /></If>
      </Dialog>
    </If>
  </View>;
}

mount(App);
