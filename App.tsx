import { cleanup, Clipboard, Image, keymap, mount, platform, Scroll, signal, Text, View, windowSize } from "nsh";
import { Button as BaseButton, Toolbar } from "nsh/base";
import { BrowserImport, BrowserStyles, DocumentSession, ImportSource, type ColumnMetadata, type ColumnProfile, type DocumentMetadata, type ImportAnalysis, type LocalImport } from "rowtation/native";
import appIcon from "./assets/icon.svg";
import { ActionList, ActionRow } from "./components/ActionList";
import { Badge } from "./components/Badge";
import { Button } from "./components/Button";
import { Choice, ChoiceGroup } from "./components/Choice";
import { DataGrid, type CellSelection } from "./components/DataGrid";
import { Dialog } from "./components/Dialog";
import { bytes, count } from "./components/format";
import { Icon } from "./components/Icon";
import { Input } from "./components/Input";
import { Inspector } from "./components/Inspector";
import { Checkbox } from "./components/Checkbox";
import { diagnosticsOn, measurements, record, timed, timeToNextFrame, wasmMemory } from "./components/diagnostics";
import { ValueDialog } from "./components/ValueDialog";

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
  const progress = signal<number | null>(null);
  const info = signal(false);
  const full = signal<string | null>(null);
  const copyLabel = signal("Copy value");
  const wide = () => windowSize().width >= 900;
  // Profiling runs in slices between frames; each new column or disposal ends the previous run.
  let run = 0;
  cleanup(() => run++);
  function ensureProfile() {
    const target = column();
    if (!target || profile()?.columnId === target.id || progress() != null) return;
    const id = ++run;
    let budget = 50_000, slices = 0, blocked = 0, longest = 0;
    const began = performance.now();
    const step = () => {
      if (id !== run) return;
      const started = performance.now();
      const result = props.document.session.profileStep(target.id, budget);
      const elapsed = performance.now() - started;
      slices++; blocked += elapsed; longest = Math.max(longest, elapsed);
      // Aim for about 12 ms of counting per slice so input and painting stay responsive.
      budget = Math.min(4_000_000, Math.max(10_000, Math.round(budget * 12 / Math.max(1, elapsed))));
      if (result.profile) {
        profile.set(result.profile); progress.set(null);
        record("Column profile", performance.now() - began, `${slices} slices, longest ${longest.toFixed(1)} ms, counting ${blocked.toFixed(1)} ms`);
      }
      else { progress.set(result.total ? result.scanned / result.total : 0); setTimeout(step, 0); }
    };
    step();
  }
  /** Stop an unfinished scan and release it in Rust; finished profiles stay cached. */
  function stopProfile() {
    run++;
    if (progress() != null) props.document.session.cancelProfile();
    progress.set(null);
  }
  /** Closing the narrow-screen inspector abandons its scan; the side panel keeps it. */
  function closeInfo() {
    info.set(false);
    if (!wide()) stopProfile();
  }
  function select(cell: CellSelection | null, columnId: number | null) {
    selection.set(cell);
    if (column()?.id === columnId) return;
    stopProfile();
    profile.set(null);
    column.set(props.document.columns.find(item => item.id === columnId) ?? null);
    if (wide() || info()) ensureProfile();
  }
  function openInfo() { info.set(true); ensureProfile(); }
  /** The selected value in full: the preview when it is whole, otherwise one readCell. */
  function wholeValue(): string | null {
    const cell = selection();
    if (!cell) return null;
    if (!cell.preview.truncated) return cell.preview.text ?? cell.preview.number ?? "";
    const value = timed("Read whole value", () => props.document.session.readCell(cell.rowId, cell.columnId), () => bytes(cell.preview.byteLength));
    return value.text ?? value.number ?? "";
  }
  function showValue() { closeInfo(); full.set(wholeValue()); }
  let copied: number | undefined;
  cleanup(() => clearTimeout(copied));
  async function copyValue() {
    const text = full() ?? wholeValue();
    if (text == null) return;
    try { await Clipboard.writeText(text); copyLabel.set("Copied"); } catch { copyLabel.set("Copy failed"); }
    clearTimeout(copied);
    copied = setTimeout(() => copyLabel.set("Copy value"), 1500);
  }
  const format = () => (props.document.metadata.delimiter === "\t" ? "TSV" : "CSV") + " · UTF-8";
  return <View class="flex-1 min-h-0 min-w-0 bg-background">
    <Toolbar accessibilityLabel="Document toolbar" class="min-h-14 shrink-0 flex-row items-center gap-3 border-b border-border/70 px-3 py-2 md:px-4">
      <Button label="Close document" quiet compact={platform.desktop} icon="caret-left" onPress={() => props.onClose()} testID="close-document" />
      <View class="flex-1 min-w-0 flex-row items-center gap-2">
        <Text class="line-clamp-1 shrink text-[15px] font-semibold text-foreground">{props.document.name}</Text>
        <Badge label={format()} />
      </View>
      <If when={wide()}><Button label="Import…" compact onPress={() => props.onImport()} /></If>
    </Toolbar>
    <View class="flex-1 min-h-0 min-w-0 flex-row">
      <View class="flex-1 min-w-0 min-h-0">
        <DataGrid session={props.document.session} metadata={props.document.metadata} columns={props.document.columns} onSelect={select} />
        <View class="shrink-0 min-h-9 flex-row items-center justify-between gap-3 border-t border-border/70 px-4 py-1.5">
          <View class="flex-row items-center gap-4">
            <Text class="text-xs text-muted-foreground">{count(props.document.metadata.rowCount, "row", "rows")} · {count(props.document.metadata.columnCount, "column", "columns")}</Text>
            <If when={selection()}><Text class="text-xs text-muted-foreground">{`Row ${(selection()?.rowIndex ?? 0) + 1} · Column ${(selection()?.columnIndex ?? 0) + 1}`}</Text></If>
          </View>
          <Text class="text-xs text-muted-foreground">Read only</Text>
        </View>
        <If when={!wide()}>
          <View class="shrink-0 flex-row items-center gap-3 border-t border-border/70 bg-muted/30 px-4 py-3">
            <BaseButton onPress={openInfo}
              class="cursor-pointer flex-1 min-h-11 flex-row items-center justify-center gap-2 rounded-full border border-border/70 bg-background px-4 shadow-sm active:bg-muted focus:ring-2 focus:ring-ring">
              <Icon name="info" class="h-4 w-4" />
              <Text class="text-sm font-medium text-foreground">Column info</Text>
            </BaseButton>
            <Button label="Import…" onPress={() => props.onImport()} />
          </View>
        </If>
      </View>
      <If when={wide()}>
        <View class="w-72 shrink-0 min-h-0 bg-muted/40 p-3">
          <View testID="inspector-panel" class="flex-1 min-h-0 overflow-hidden rounded-2xl border border-border/60 bg-background shadow-sm">
            <Scroll testID="inspector-scroll" class="flex-1 min-h-0">
              <Inspector column={column()} profile={profile()} metadata={props.document.metadata} selection={selection()} progress={progress()}
                copyLabel={copyLabel()} onShowValue={showValue} onCopyValue={() => void copyValue()} />
            </Scroll>
          </View>
        </View>
      </If>
    </View>
    <If when={info() && windowSize().width < 900}>
      <Dialog title="Column information" onClose={closeInfo}>
        <Scroll style={{ maxHeight: windowSize().height * 0.6 }} class="min-h-0"><Inspector column={column()} profile={profile()} metadata={props.document.metadata} selection={selection()} progress={progress()}
          copyLabel={copyLabel()} onShowValue={showValue} onCopyValue={() => void copyValue()} /></Scroll>
        <Button label="Done" compact={platform.desktop} onPress={closeInfo} />
      </Dialog>
    </If>
    <If when={full() != null}>
      <ValueDialog text={full() ?? ""} byteLength={selection()?.preview.byteLength ?? 0} copyLabel={copyLabel()}
        title={`Row ${(selection()?.rowIndex ?? 0) + 1} · ${column()?.name || "(unnamed)"}`}
        onCopy={() => void copyValue()} onClose={() => full.set(null)} />
    </If>
  </View>;
}

function DiagnosticsPanel() {
  const memory = () => { measurements(); return wasmMemory(); };
  return <View testID="diagnostics" class="absolute bottom-3 right-3 w-80 max-w-[92%] gap-1 rounded-xl border border-border bg-background p-3 shadow-lg">
    <Text class="text-xs font-semibold text-foreground">Diagnostics · local only</Text>
    <Text class="text-xs text-muted-foreground">{`WebAssembly memory ${bytes(memory())} (peak, never shrinks)`}</Text>
    <Each of={measurements()}>{item => <Text class="text-xs text-foreground">{`${item().label}: ${item().ms.toFixed(1)} ms${item().detail ? ` · ${item().detail}` : ""}`}</Text>}</Each>
    <Text class="text-xs text-muted-foreground">{`${keymap.label("ctrl+shift+d")} hides this panel and stops measuring.`}</Text>
  </View>;
}

function Home(props: { dragging: boolean; onPick: () => void; onPaste: () => void }) {
  keymap.bind("mod+v", () => props.onPaste(), { when: "notEditing" });
  const note = "Your data stays in this browser. Nothing is uploaded.";
  const formats = <View class="flex-row gap-2">
    <Each of={["CSV", "TSV"]}>{label => <View class="rounded-full border border-border/60 bg-background px-2.5 py-1 shadow-sm">
      <Text class="text-xs font-medium text-foreground">{label()}</Text></View>}</Each>
  </View>;
  return <Scroll class="flex-1 min-h-0" contentStyle={{ flexGrow: 1 }}>
    <If when={windowSize().width >= 640} else={
      <View class="flex-1 justify-center px-5 py-10">
        <View class="w-full max-w-[560px] gap-6 self-center">
          <Text class="text-4xl font-bold tracking-tight text-foreground">Rowtation</Text>
          <BaseButton testID="import-area" onPress={() => props.onPick()}
            class="cursor-pointer min-h-56 items-center justify-center gap-3 rounded-3xl border border-border/70 bg-background p-8 shadow-sm active:bg-muted/40 focus:ring-2 focus:ring-ring">
            <Icon name="upload-simple" class="h-8 w-8" />
            <Text class="pt-1 text-lg font-semibold text-foreground">Import data</Text>
            <Text class="text-[13px] text-muted-foreground">CSV · TSV</Text>
          </BaseButton>
          <ActionList>
            <ActionRow label="Paste from clipboard" icon="clipboard-text" onPress={() => props.onPaste()} />
            <ActionRow label="Choose file…" icon="folder-open" onPress={() => props.onPick()} last />
          </ActionList>
          <Text class="px-1 text-xs text-muted-foreground">{note}</Text>
        </View>
      </View>}>
      <View class="flex-1 items-center justify-center px-8 py-16">
        <View class="w-full max-w-[600px] items-center gap-8">
          <View class="items-center gap-4">
            <Image src={appIcon} accessibilityElementsHidden class="h-16 w-16" />
            <View class="items-center gap-1.5">
              <Text class="text-3xl font-semibold tracking-tight text-foreground">Rowtation</Text>
              <Text class="text-sm text-muted-foreground">Import and inspect CSV and TSV.</Text>
            </View>
          </View>
          <View testID="import-area" classList={{ "ring-2 ring-ring": props.dragging }}
            class="w-full min-h-64 items-center justify-center gap-3 rounded-3xl border border-border/60 bg-background p-10 shadow-md">
            <Icon name="upload-simple" class="h-7 w-7" />
            <Text class="pt-1 text-base font-semibold text-foreground">Drop a file here</Text>
            <Text class="text-[13px] text-muted-foreground">{`Or paste with ${keymap.label("mod+v")}, or choose a file from this computer.`}</Text>
            <View class="flex-row gap-2 pt-1">
              <Button label="Paste from clipboard" quiet compact onPress={() => props.onPaste()} />
              <Button label="Choose file…" compact onPress={() => props.onPick()} />
            </View>
            <View class="pt-3">{formats}</View>
          </View>
          <Text class="text-xs text-center text-muted-foreground">{note}</Text>
        </View>
      </View>
    </If>
  </Scroll>;
}

export function App() {
  // NSH 0.4 draws a desktop bar for horizontal scroll views even when their
  // indicator is false. DataTable already supplies a bottom viewport bar.
  // Scope this presentation workaround to that table's disabled inner indicator.
  cleanup(BrowserStyles.install('[data-testid="data-grid"] .nsh-noindicator > .nsh-bar { display: none; }'));

  const active = signal<OpenDocument | null>(null);
  const mode = signal<"paste" | "confirm" | "review" | null>(null);
  // The reviewed bytes stay in Rust; this handle is disposed when review ends.
  const draft = signal<ImportSource | null>(null);
  // A large file waiting for confirmation before the browser reads it.
  const pending = signal<LocalImport | null>(null);
  const accepted = signal(false);
  const paste = signal("");
  const delimiter = signal(",");
  const headers = signal(true);
  const analysis = signal<ImportAnalysis | null>(null);
  const error = signal("");
  const busy = signal(false);
  const dragging = signal(false);
  let alive = true;
  let request = 0;
  cleanup(() => { alive = false; request++; BrowserImport.cancelPending(); draft()?.dispose(); active()?.dispose(); });
  keymap.bind("ctrl+shift+d", () => { diagnosticsOn.set(!diagnosticsOn()); measurements.set([]); });

  /** Clears review state; the Rust-held file is released here or by cancelPending. */
  function clearDraft() {
    request++;
    draft()?.dispose();
    draft.set(null); pending.set(null); analysis.set(null); busy.set(false); error.set(""); accepted.set(false);
  }
  function resetImport() { clearDraft(); BrowserImport.cancelPending(); }
  function dismiss() { resetImport(); mode.set(null); paste.set(""); }
  function message(cause: unknown) { return cause instanceof Error ? cause.message : String(cause); }
  function fail(value: unknown) { error.set(message(value)); }
  function inspect(source: ImportSource, retry: "paste" | null) {
    try {
      const result = timed("Inspect", () => source.inspect(), () => bytes(source.byteLength));
      draft.set(source); analysis.set(result); delimiter.set(result.delimiter); headers.set(true); mode.set("review");
    } catch (cause) {
      const name = source.name;
      source.dispose();
      mode.set(retry);
      error.set(`${name}: ${message(cause)}`);
    }
  }
  function reviewPaste() {
    const source = ImportSource.fromText(paste(), "Pasted data");
    clearDraft();
    inspect(source, "paste");
  }
  function receive(file: LocalImport) {
    const confirmed = accepted();
    // Take the read file before clearing, or wait for confirmation without reading.
    let source: ImportSource | null = null, failure = file.error ?? "";
    if (!failure && !file.deferred) { try { source = BrowserImport.take(file.requestId!); } catch (cause) { failure = message(cause); } }
    clearDraft();
    if (file.deferred) { pending.set(file); mode.set("confirm"); return; }
    if (!source) { mode.set(null); error.set(file.name ? `${file.name}: ${failure}` : failure); return; }
    record("File read", file.readMs, bytes(file.size));
    accepted.set(confirmed);
    inspect(source, null);
  }
  cleanup(BrowserImport.onFile(file => {
    if (alive && file.requestId != null && BrowserImport.isCurrent(file.requestId)) receive(file);
  }, active => { if (alive) dragging.set(active); }));
  function readLarge() {
    const file = pending();
    if (mode() !== "confirm" || !file || busy()) return;
    busy.set(true); error.set(""); accepted.set(true);
    try { BrowserImport.readDeferred(file.requestId!); } catch (cause) { busy.set(false); fail(cause); }
  }

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
    const source = draft();
    if (mode() !== "review" || !source || busy()) return;
    const id = ++request;
    const options = { delimiter: delimiter(), headers: headers() };
    busy.set(true); error.set("");
    // Let the loading state paint before the synchronous Rust parser runs.
    await new Promise(resolve => setTimeout(resolve, 30));
    if (!alive || id !== request) return;
    let candidate: DocumentSession | null = null;
    try {
      candidate = timed("Parse", () => DocumentSession.fromSource(source, options), () => bytes(source.byteLength));
      const session = candidate;
      const metadata = session.metadata();
      const columns = session.columns();
      let live = true;
      const document: OpenDocument = { session, metadata, columns, name: source.name,
        dispose: () => { if (live) { live = false; session.dispose(); } } };
      active.set(document);
      timeToNextFrame("Document open to next frame", `${metadata.columnCount} columns`);
      // The session holds its own data; release the source bytes now.
      source.dispose(); draft.set(null); analysis.set(null); paste.set(""); mode.set(null);
    } catch (cause) { candidate?.dispose(); fail(cause); }
    finally { busy.set(false); }
  }

  const compact = platform.desktop;
  const size = () => { const result = analysis(); return result ? (delimiter() === "\t" ? result.tab : result.comma) : null; };
  const summary = () => {
    const result = analysis(), measured = size();
    if (!result || !measured) return "";
    const rows = Math.max(0, measured.records - (headers() ? 1 : 0));
    return `${bytes(result.byteLength)} · ${count(rows, "row", "rows")} · ${count(measured.columns, "column", "columns")} · longest field ${bytes(measured.longestField)}`;
  };

  return <View class="flex-1 min-h-0 min-w-0 bg-muted/40 pt-[env(safe-area-inset-top)] pr-[env(safe-area-inset-right)] pb-[env(safe-area-inset-bottom)] pl-[env(safe-area-inset-left)]">
    <Each of={active() ? [active()!] : []} key={document => document.metadata.id}>
      {document => <DocumentView document={document()} onClose={() => { dismiss(); active.set(null); }} onImport={openPaste} />}
    </Each>
    <If when={!active()}>
      <Home dragging={dragging()} onPick={pick} onPaste={() => { if (!mode()) void openPaste(); }} />
    </If>
    <If when={error() && !mode()}>
      <View role="alert" testID="import-error"
        style={windowSize().width < 640 ? { position: "absolute", left: 12, right: 12, bottom: 12 }
          : { position: "absolute", left: "50%", bottom: 24, translate: "-50% 0" }}
        class={(windowSize().width < 640 ? "" : "w-[480px] max-w-[92%] ") + "flex-row items-start gap-3 rounded-2xl border border-border/70 bg-background p-4 shadow-lg"}>
        <Icon name="warning" class="mt-0.5 h-5 w-5" />
        <View class="flex-1 gap-1 pt-0.5">
          <Text class="text-sm font-semibold text-foreground">Import failed</Text>
          <Text class="text-[13px] text-muted-foreground">{error()}</Text>
        </View>
        <Button label="Dismiss" icon="x" quiet compact onPress={() => error.set("")} />
      </View>
    </If>
    {/* Keyed by step so review opens at the top instead of keeping the paste step's scroll offset. */}
    <Each of={mode() ? [mode()!] : []} key={step => step}>{() =>
      <Dialog title={mode() === "paste" ? "Paste your data" : `Import “${(mode() === "confirm" ? pending()?.name : draft()?.name) ?? ""}”`}
        description={mode() === "paste" ? "Paste CSV or TSV text below. You can review the format before opening it."
          : mode() === "confirm" ? `This file is ${bytes(pending()?.size ?? 0)}. Rowtation has not read it yet.`
          : analysis()?.ambiguous ? "Confirm the separator below; detection is uncertain." : "Format detected. Review the options before importing."}
        onClose={dismiss}>
        <If when={mode() === "paste"}>
          <Input label="CSV or TSV text" testID="paste-input" value={paste()} onChange={value => paste.set(value)} multiline placeholder={"name,city\nMia,Brisbane"} />
        </If>
        <If when={mode() === "confirm"}>
          <View role="alert" class="flex-row gap-3 rounded-xl border border-border bg-muted/60 p-4">
            <Icon name="warning" class="h-5 w-5" />
            <View class="flex-1 gap-1">
              <Text class="text-sm font-semibold text-foreground">This is a large file</Text>
              <Text class="text-[13px] text-foreground">Reading and importing it may take a long time, make this tab unresponsive, or run out of browser memory. Read it only if you want to continue.</Text>
            </View>
          </View>
        </If>
        <If when={mode() === "review"}>
          <ChoiceGroup>
            <Choice mark="CSV" label="Comma" checked={delimiter() === ","} disabled={busy()} onSelect={() => delimiter.set(",")} />
            <Choice mark="TSV" label="Tab" checked={delimiter() === "\t"} disabled={busy()} onSelect={() => delimiter.set("\t")} />
          </ChoiceGroup>
          <View class="rounded-xl border border-border/70 px-4">
            <Checkbox label="First row contains column headers" checked={headers()} disabled={busy()} onChange={value => headers.set(value)} />
          </View>
          <View class="gap-1">
            <Text testID="import-summary" class="text-[13px] text-foreground">{summary()}</Text>
            <Text class="text-xs text-muted-foreground">Values are preserved as text, including leading zeros and formula-like content.</Text>
          </View>
          <If when={size()?.large && !accepted()}>
            <View role="alert" class="flex-row gap-3 rounded-xl border border-border bg-muted/60 p-4">
              <Icon name="warning" class="h-5 w-5" />
              <View class="flex-1 gap-1">
                <Text class="text-sm font-semibold text-foreground">This is a large dataset</Text>
                <Text class="text-[13px] text-foreground">Importing it may take a long time, make this tab unresponsive, or run out of browser memory. Import only if you want to continue.</Text>
              </View>
            </View>
          </If>
        </If>
        <If when={error()}><View role="alert" class="gap-2"><Text class="text-sm text-destructive">{error()}</Text>
          <Text class="text-xs text-muted-foreground">Check the separator and quoted fields, or choose another file. Your current document is unchanged.</Text></View></If>
        <View class="flex-row flex-wrap items-center justify-end gap-2">
          <If when={mode() === "paste" || (error() && !busy())}><Button label="Choose another file…" quiet compact={compact} onPress={pick} /></If>
          <View class="flex-1" />
          <Button label="Cancel" quiet compact={compact} onPress={dismiss} />
          <If when={mode() === "paste"}>
            <Button label="Review import" compact={compact} disabled={!paste()} onPress={reviewPaste} />
          </If>
          <If when={mode() === "confirm"}>
            <Button label={busy() ? "Reading…" : "Read file"} compact={compact} disabled={busy() || !pending()} onPress={readLarge} />
          </If>
          <If when={mode() === "review"}>
            <Button label={busy() ? "Importing…" : size()?.large && !accepted() ? "Import anyway" : "Import document"} compact={compact} disabled={busy() || !draft()} onPress={importDocument} />
          </If>
        </View>
      </Dialog>}
    </Each>
    <If when={diagnosticsOn()}><DiagnosticsPanel /></If>
  </View>;
}

mount(App);
