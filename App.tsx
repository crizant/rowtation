import { cleanup, Clipboard, effect, Image, keymap, mount, platform, Scroll, signal, Text, View, windowSize, type InputRef } from "nsh";
import { AlertDialog, Button as BaseButton, Menu, MenuItem, MenuSeparator, Toolbar } from "nsh/base";
import { BrowserImport, BrowserStyles, DocumentSession, ImportSource, type ColumnMetadata, type ColumnProfile, type DocumentMetadata, type ImportAnalysis, type ImportSize, type LocalImport, type SearchMatch, type SearchResult } from "rowtation/native";
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
import { PREVIEW_BYTES } from "./components/gridRowCache";
import { ExportDialog } from "./components/ExportDialog";
import { ValueDialog } from "./components/ValueDialog";

interface OpenDocument {
  session: DocumentSession;
  metadata: DocumentMetadata;
  columns: ColumnMetadata[];
  name: string;
  dispose: () => void;
}

function MenuRow(props: { label: string; disabled?: boolean; onSelect: () => void }) {
  return <MenuItem disabled={props.disabled} onSelect={() => props.onSelect()}
    class="min-h-11 cursor-pointer flex-row items-center rounded-lg px-3 hover:bg-muted active:bg-muted focus:bg-muted disabled:cursor-not-allowed disabled:opacity-50 md:min-h-9">
    <Text class="text-sm text-foreground">{props.label}</Text>
  </MenuItem>;
}

function EditInput(props: { value: string; onChange: (value: string) => void; onCommit: (tab?: boolean) => void; onCancel: () => void; onBind: (blur: () => void) => void }) {
  let input: InputRef | undefined;
  props.onBind(() => input?.blur());
  function commit(tab?: boolean) { input?.blur(); props.onCommit(tab); }
  function cancel() { input?.blur(); props.onCancel(); }
  // NSH Input does not report Tab through onKeyPress; this binding lives only with the editor.
  keymap.bind("tab", () => commit(true), { when: "always" });
  keymap.bind("escape", () => cancel(), { when: "always" });
  return <Input ref={node => { input = node; }} label="Cell value" testID="cell-draft" value={props.value} onChange={value => props.onChange(value)} autoFocus multiline
    submitBehavior="submit" onSubmit={() => commit()} />;
}

function DocumentView(props: { document: OpenDocument; onClose: () => void; onImport: () => void }) {
  cleanup(() => props.document.dispose());
  const metadata = signal(props.document.metadata);
  const version = signal(0);
  const edits = signal(props.document.session.editStatus());
  const sorting = signal<import("nsh/base").DataTableSort | null>(null);
  const finding = signal(false);
  const replacing = signal(false);
  const menuPoint = signal<{ x: number; y: number } | null>(null);
  const term = signal("");
  const replacement = signal("");
  const sensitive = signal(false);
  const regex = signal(false);
  const scoped = signal(false);
  const scopeId = signal<number | null>(null);
  const results = signal<SearchResult>({ count: 0, current: null });
  const matchIndex = signal(0);
  const activeMatch = signal<SearchMatch | null>(null);
  const activeSnippet = signal("");
  const querySelection = signal<{ start: number; end: number } | undefined>(undefined);
  const pending = signal(false);
  const operationError = signal("");
  const notice = signal("");
  const exporting = signal(false);
  const editing = signal(false);
  const editDraft = signal("");
  let focusGrid = () => {};
  let findInput: InputRef | undefined;
  let selectQuery = false;
  let searchTimer: ReturnType<typeof setTimeout> | undefined;
  let focusTimer: ReturnType<typeof setTimeout> | undefined;
  let appliedKey = "";
  let held: { rowId: number; columnId: number; start: number } | null = null;
  let heldIndex = 0;
  cleanup(() => { clearTimeout(searchTimer); clearTimeout(focusTimer); });
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
    if (!cell.preview.truncated) return cell.preview.text ?? cell.preview.number ?? (cell.preview.boolean == null ? "" : String(cell.preview.boolean));
    const value = timed("Read whole value", () => props.document.session.readCell(cell.rowId, cell.columnId), () => bytes(cell.preview.byteLength));
    return value.text ?? value.number ?? (value.boolean == null ? "" : String(value.boolean));
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
  function attempt(action: () => void) {
    operationError.set(""); notice.set("");
    try { action(); } catch (cause) { operationError.set(cause instanceof Error ? cause.message : String(cause)); }
  }
  function refresh() {
    metadata.set(props.document.session.metadata());
    edits.set(props.document.session.editStatus());
    const current = selection();
    if (current) selection.set({ ...current, rowIndex: props.document.session.rowPosition(current.rowId), preview: props.document.session.readCell(current.rowId, current.columnId) });
    stopProfile(); profile.set(null);
    if (wide() || info()) ensureProfile();
    runSearch(finding());
  }
  function queryKey() {
    return `${term()}\0${sensitive()}\0${regex()}\0${scoped() ? scopeId() ?? "" : ""}`;
  }
  function resolveIndex(count: number) {
    const fallback = Math.min(heldIndex, Math.max(count - 1, 0));
    if (!held || count < 1) return fallback;
    return props.document.session.searchIndex(held.rowId, held.columnId, held.start, heldIndex) ?? fallback;
  }
  function showSnippet(index: number) {
    activeSnippet.set(props.document.session.searchSnippet(index, PREVIEW_BYTES) ?? "");
  }
  function remember(index: number) {
    const match = props.document.session.searchMatch(index);
    if (!match) return null;
    heldIndex = index;
    held = { rowId: match.rowId, columnId: match.columnId, start: match.start };
    matchIndex.set(index);
    activeMatch.set(match);
    return match;
  }
  function runSearch(reveal = true, mode: "preserve" | "restart" = "preserve") {
    clearTimeout(searchTimer);
    searchTimer = undefined;
    pending.set(false);
    const activeTerm = finding() ? term() : "";
    try {
      const result = props.document.session.search({
        term: activeTerm,
        columnId: activeTerm && scoped() ? scopeId() : null,
        caseSensitive: sensitive(),
        regex: regex(),
      });
      appliedKey = queryKey();
      operationError.set("");
      results.set(result);
      if (!result.count) {
        matchIndex.set(0);
        activeMatch.set(null);
        activeSnippet.set("");
        if (mode === "restart" || !activeTerm) { held = null; heldIndex = 0; }
      } else if (reveal) revealMatch(mode === "restart" ? 0 : resolveIndex(result.count));
      else {
        const index = mode === "restart" ? 0 : resolveIndex(result.count);
        if (remember(index)) showSnippet(index);
      }
    } catch (cause) {
      results.set({ count: 0, current: null });
      matchIndex.set(0);
      activeMatch.set(null);
      activeSnippet.set("");
      if (mode === "restart") { held = null; heldIndex = 0; }
      appliedKey = queryKey();
      operationError.set(cause instanceof Error ? cause.message : String(cause));
    }
    version.set(version() + 1);
  }
  function queueSearch() {
    clearTimeout(searchTimer);
    pending.set(true);
    operationError.set("");
    searchTimer = setTimeout(() => { searchTimer = undefined; attempt(() => runSearch(true, "restart")); }, 180);
  }
  function revealMatch(index: number) {
    if (pending()) return;
    const length = results().count;
    if (!length) return;
    const next = (index % length + length) % length;
    const match = remember(next);
    if (!match) return;
    showSnippet(next);
    const columnIndex = props.document.columns.findIndex(c => c.id === match.columnId);
    if (columnIndex < 0) return;
    const row = props.document.session.rows({ offset: match.rowIndex, limit: 1, columnOffset: columnIndex, columnLimit: 1, previewBytes: PREVIEW_BYTES }).rows[0];
    const preview = row?.cells[0];
    if (!preview) return;
    select({ documentId: metadata().id, rowId: match.rowId, columnId: match.columnId, rowIndex: match.rowIndex, columnIndex, preview }, match.columnId);
  }
  function searchStatus() {
    if (!term()) return "";
    if (pending()) return "Searching…";
    if (operationError()) return "";
    if (!results().count) return "No matches";
    return `${matchIndex() + 1} of ${results().count} matches`;
  }
  function sort(next: import("nsh/base").DataTableSort | null) {
    attempt(() => {
      props.document.session.sort(next ? Number(next.key) : null, next?.direction === "descending");
      sorting.set(next); refresh();
    });
  }
  function history(redo: boolean) { if (exporting()) return; attempt(() => { props.document.session.history(redo); refresh(); }); }
  function beginEdit() { if (exporting()) return; if (selection()) { editDraft.set(wholeValue() ?? ""); editing.set(true); } }
  function editAt(rowIndex: number, columnIndex: number) {
    const row = props.document.session.rows({ offset: rowIndex, limit: 1, columnOffset: columnIndex, columnLimit: 1 }).rows[0];
    if (!row) return;
    const columnId = props.document.columns[columnIndex].id;
    select({ documentId: metadata().id, rowId: row.id, columnId, rowIndex, columnIndex, preview: row.cells[0] }, columnId);
    beginEdit();
  }
  function openCellMenu(rowIndex: number, columnIndex: number, x: number, y: number) {
    const row = props.document.session.rows({ offset: rowIndex, limit: 1, columnOffset: columnIndex, columnLimit: 1 }).rows[0];
    if (!row) return;
    const columnId = props.document.columns[columnIndex].id;
    select({ documentId: metadata().id, rowId: row.id, columnId, rowIndex, columnIndex, preview: row.cells[0] }, columnId);
    menuPoint.set({ x, y });
  }
  function confirmDiscard(action: () => void) {
    const modified = edits().modifiedCells;
    if (!modified) { action(); return; }
    void AlertDialog.show({
      title: "Discard unsaved changes?",
      description: `${count(modified, "modified cell", "modified cells")} will be dropped. Export before closing if you need a copy. Downloads do not reset the imported baseline.`,
      closeOnEscape: true,
      actions: [
        { id: "keep", label: "Keep editing", role: "cancel" },
        { id: "discard", label: "Discard changes", role: "destructive" },
      ],
    }).then(result => { if (result.action === "discard") action(); });
  }
  function focusQuery() {
    const input = findInput;
    if (!input) return;
    selectQuery = false;
    const length = term().length;
    querySelection.set({ start: 0, end: length });
    input.focus();
    input.setSelection(0, length);
  }
  function openFind() {
    if (editing() || exporting()) return;
    selectQuery = true;
    const wasOpen = finding();
    finding.set(true);
    if (!wasOpen && term()) attempt(() => runSearch(true));
    if (wasOpen) focusQuery();
  }
  function closeFind() {
    findInput?.blur();
    finding.set(false);
    replacing.set(false);
    attempt(() => runSearch(false));
    clearTimeout(focusTimer);
    focusGrid();
  }
  function toggleFind() { if (finding()) closeFind(); else openFind(); }
  function stepMatch(delta: number) {
    if (!finding() || editing() || pending()) return;
    if (queryKey() !== appliedKey) {
      attempt(() => runSearch(true, "restart"));
      if (delta < 0 && results().count > 1) revealMatch(results().count - 1);
      return;
    }
    revealMatch(matchIndex() + delta);
  }
  function toggleOption(kind: "case" | "regex" | "scope") {
    if (kind === "case") sensitive.set(!sensitive());
    else if (kind === "regex") regex.set(!regex());
    else if (scoped()) scoped.set(false);
    else {
      const id = column()?.id;
      if (id == null) return;
      scopeId.set(id);
      scoped.set(true);
    }
    attempt(() => runSearch(true));
  }
  let blurEditor = () => {};
  function releaseEditor(after?: () => void) {
    blurEditor();
    clearTimeout(focusTimer);
    focusTimer = setTimeout(() => {
      editing.set(false);
      after?.();
      focusGrid();
    }, 0);
  }
  function cancelEdit() { releaseEditor(); }
  function commitEdit(tab = false) {
    attempt(() => {
      const current = selection(); if (!current) return;
      if (editDraft() !== wholeValue()) props.document.session.editCell(current.rowId, current.columnId, editDraft(), metadata().revision);
      const position = current.rowIndex * metadata().columnCount + current.columnIndex + 1;
      releaseEditor(() => {
        refresh();
        if (tab && metadata().columnCount) {
          const rowIndex = Math.floor(position / metadata().columnCount), columnIndex = position % metadata().columnCount;
          if (rowIndex < metadata().rowCount) {
            const row = props.document.session.rows({ offset: rowIndex, limit: 1, columnOffset: columnIndex, columnLimit: 1 }).rows[0];
            const columnId = props.document.columns[columnIndex].id;
            select({ documentId: metadata().id, rowId: row.id, columnId, rowIndex, columnIndex, preview: row.cells[0] }, columnId);
          }
        }
      });
    });
  }
  function clearCell() { attempt(() => { const cell = selection(); if (cell) { props.document.session.editCell(cell.rowId, cell.columnId, "", metadata().revision); refresh(); } }); }
  function replace(all: boolean) {
    attempt(() => {
      if (searchTimer || queryKey() !== appliedKey) runSearch(true, "restart");
      if (!results().count || operationError()) return;
      const result = props.document.session.replaceMatches(all ? null : matchIndex(), replacement(), metadata().revision);
      refresh(); notice.set(`${count(result.replacements, "replacement", "replacements")} in ${count(result.affectedCells, "cell", "cells")}`);
    });
  }
  keymap.bind("mod+z", () => history(false), { when: "notEditing" });
  keymap.bind("mod+shift+z", () => history(true), { when: "notEditing" });
  keymap.bind("f2", () => { if (finding() || editing()) return; beginEdit(); });
  keymap.bind("mod+f", openFind);
  effect(() => {
    if (!finding() || exporting()) return;
    if (editing()) {
      keymap.bind("shift+enter", () => editDraft.set(`${editDraft()}\n`), { when: "always" });
      return;
    }
    keymap.bind("enter", () => stepMatch(1), { when: "always" });
    keymap.bind("shift+enter", () => stepMatch(-1), { when: "always" });
    keymap.bind("escape", closeFind, { when: "always" });
  });
  function scopeLabel() {
    if (!scoped()) return "All";
    const name = props.document.columns.find(item => item.id === scopeId())?.name || "Column";
    return name.length > 14 ? `${name.slice(0, 13)}…` : name;
  }
  function gridMatch() {
    const match = activeMatch();
    if (!match || !results().count) return null;
    const column = props.document.columns.findIndex(item => item.id === match.columnId);
    if (column < 0) return null;
    return { row: match.rowIndex, column, start: match.start, end: match.end };
  }
  const format = () => props.document.metadata.format.toUpperCase() + " · UTF-8";
  const menuPopup = "min-w-44 overflow-hidden rounded-xl border border-border/70 bg-background p-1 shadow-lg";
  return <View class="relative flex-1 min-h-0 min-w-0 bg-background">
    <Toolbar accessibilityLabel="Document toolbar" class="min-h-14 shrink-0 flex-row items-center gap-2 border-b border-border/70 px-3 py-2 md:gap-3 md:px-4">
      <Button label="Close document" quiet compact={platform.desktop} icon="caret-left" onPress={() => confirmDiscard(() => props.onClose())} testID="close-document" />
      <View class={(wide() ? "flex-row items-center gap-2 " : "justify-center gap-0.5 ") + "min-w-0 flex-1"}>
        <Text class="line-clamp-1 text-[15px] font-semibold text-foreground">{props.document.name}</Text>
        <If when={wide()}><Badge label={format()} /></If>
        <If when={!wide()}><Text class="line-clamp-1 text-[11px] font-medium text-muted-foreground">{format()}</Text></If>
      </View>
      <Button label={finding() ? "Close find" : "Find"} quiet compact={wide()} icon={finding() ? "x" : "magnifying-glass"} onPress={toggleFind}
        tooltip={finding() ? undefined : `Find ${keymap.label("mod+f")}`} />
      <Button label="Undo" quiet compact={wide()} icon="arrow-counter-clockwise" disabled={!edits().canUndo} onPress={() => history(false)} tooltip={`Undo ${keymap.label("mod+z")}`} />
      <Button label="Redo" quiet compact={wide()} icon="arrow-clockwise" disabled={!edits().canRedo} onPress={() => history(true)} tooltip={`Redo ${keymap.label("mod+shift+z")}`} />
      <If when={wide()}><Button label="Export…" compact onPress={() => exporting.set(true)} /></If>
      <Menu popupClass={menuPopup} backdropClass="bg-foreground/20" trigger={<Button label="More" quiet compact={wide()} icon="dots-three" onPress={() => {}} />}>
        <MenuRow label="Import…" onSelect={() => confirmDiscard(() => props.onImport())} />
      </Menu>
    </Toolbar>
    <If when={finding()}>
      <View class="shrink-0 gap-2 border-b border-border/70 px-3 py-2">
        {/* Narrow screens keep the query on its own row so the field stays usable. */}
        <View class={wide() ? "flex-row items-center gap-2" : "gap-2"}>
          <View class={"flex-row items-center gap-2" + (wide() ? " min-w-0 flex-1" : "")}>
            <View class="min-w-0 flex-1"><Input label="Find" placeholder="Find" testID="find-input" autoFocus value={term()} selectedRange={querySelection()} ref={node => { findInput = node; if (selectQuery) focusQuery(); }}
              onChange={value => { querySelection.set(undefined); term.set(value); if (!value) { clearTimeout(searchTimer); searchTimer = undefined; pending.set(false); attempt(() => runSearch(false)); return; } queueSearch(); }} /></View>
            <Button label="Previous" icon="caret-left" quiet compact={wide()} disabled={pending() || !results().count} onPress={() => stepMatch(-1)} />
            <Button label="Next" icon="caret-right" quiet compact={wide()} disabled={pending() || !results().count} onPress={() => stepMatch(1)} />
          </View>
          <View class="flex-row flex-wrap items-center gap-2">
            <Text testID="search-count" class={(wide() ? "shrink-0" : "min-w-0 flex-1") + " line-clamp-1 text-xs text-muted-foreground"}>{searchStatus()}</Text>
            <If when={activeSnippet()}><Text testID="active-match" class="max-w-36 line-clamp-1 text-xs font-medium text-foreground underline" style={{ textDecorationLine: "underline" }}>{activeSnippet()}</Text></If>
            <Button label="Aa" accessibilityLabel="Case sensitive" tight quiet compact={wide()} selected={sensitive()} tooltip="Case sensitive" onPress={() => toggleOption("case")} />
            <Button label=".*" accessibilityLabel="Regex" tight quiet compact={wide()} selected={regex()} tooltip="Regex" onPress={() => toggleOption("regex")} />
            <Button label={scopeLabel()} accessibilityLabel={scoped() ? "Selected column" : "All columns"} tight quiet compact={wide()} selected={scoped()} disabled={!column() && !scoped()} tooltip={scoped() ? "Selected column" : "All columns"} onPress={() => toggleOption("scope")} />
            <Button label={replacing() ? "Hide replace" : "Replace…"} quiet compact={wide()} onPress={() => replacing.set(!replacing())} />
          </View>
        </View>
        <If when={replacing()}>
          <View class={wide() ? "flex-row items-center gap-2" : "gap-2"}>
            <View class="min-w-0 flex-1"><Input label="Replace with" placeholder="Replace with" testID="replace-input" value={replacement()} onChange={value => replacement.set(value)} /></View>
            <View class="flex-row items-center gap-2">
              <Button label="Replace" quiet compact={wide()} disabled={!results().count || !!operationError()} onPress={() => replace(false)} />
              <Button label="Replace all" compact={wide()} disabled={!results().count || !!operationError()} onPress={() => replace(true)} />
            </View>
          </View>
        </If>
        <If when={replacing() && (metadata().format === "json" || metadata().format === "ndjson")}><Text class="text-xs text-muted-foreground">Changed numbers and booleans become text. Unchanged values keep their type.</Text></If>
        <If when={regex()}><Text class="text-xs text-muted-foreground">Replacement is literal</Text></If>
        <If when={operationError()}><Text role="alert" class="text-xs text-destructive">{operationError()}</Text></If>
        <If when={notice()}><Text class="text-xs text-muted-foreground">{notice()}</Text></If>
      </View>
    </If>
    <View class="flex-1 min-h-0 min-w-0 flex-row">
      <View class="flex-1 min-w-0 min-h-0">
        <DataGrid session={props.document.session} metadata={metadata()} columns={props.document.columns} onSelect={select}
          version={version()} selected={selection()} sort={sorting()} onSort={sort} onEdit={editAt} onCellMenu={openCellMenu} activeMatch={gridMatch()} onReady={focus => { focusGrid = focus; }} />
        <View class="shrink-0 min-h-9 flex-row items-center justify-between gap-3 border-t border-border/70 px-4 py-1.5">
          <View class="flex-1 min-w-0 flex-row flex-wrap items-center gap-x-4 gap-y-1">
            <Text class="text-xs text-muted-foreground">{count(props.document.metadata.rowCount, "row", "rows")} · {count(props.document.metadata.columnCount, "column", "columns")}</Text>
            <If when={selection()}><Text class="text-xs text-muted-foreground">{`Row ${(selection()?.rowIndex ?? 0) + 1} · Column ${(selection()?.columnIndex ?? 0) + 1}`}</Text></If>
          </View>
          <Text testID="edit-status" class="text-xs text-muted-foreground">{edits().modifiedCells ? `${edits().modifiedCells} modified · Unsaved` : "Unmodified"}</Text>
        </View>
        <If when={!wide()}>
          <View class="shrink-0 flex-row items-center gap-3 border-t border-border/70 bg-muted/30 px-4 py-3">
            <BaseButton onPress={openInfo}
              class="cursor-pointer flex-1 min-h-11 flex-row items-center justify-center gap-2 rounded-full border border-border/70 bg-background px-4 shadow-sm active:bg-muted focus:ring-2 focus:ring-ring">
              <Icon name="info" class="h-4 w-4" />
              <Text class="text-sm font-medium text-foreground">Column info</Text>
            </BaseButton>
            <Button label="Import…" quiet onPress={() => confirmDiscard(() => props.onImport())} />
            <Button label="Export…" onPress={() => exporting.set(true)} />
          </View>
        </If>
      </View>
      <If when={wide()}>
        <View class="w-72 shrink-0 min-h-0 bg-muted/40 p-3">
          <View testID="inspector-panel" class="flex-1 min-h-0 overflow-hidden rounded-2xl border border-border/60 bg-background shadow-sm">
            <Scroll testID="inspector-scroll" class="flex-1 min-h-0">
              <Inspector column={column()} profile={profile()} metadata={metadata()} selection={selection()} progress={progress()}
                copyLabel={copyLabel()} onShowValue={showValue} onCopyValue={() => void copyValue()} />
            </Scroll>
          </View>
        </View>
      </If>
    </View>
    <If when={exporting()}><ExportDialog session={props.document.session} name={props.document.name} onClose={() => exporting.set(false)} /></If>
    <If when={info() && windowSize().width < 900}>
      <Dialog title="Column information" onClose={closeInfo}>
        <Scroll style={{ maxHeight: windowSize().height * 0.6 }} class="min-h-0"><Inspector column={column()} profile={profile()} metadata={metadata()} selection={selection()} progress={progress()}
          copyLabel={copyLabel()} onShowValue={showValue} onCopyValue={() => void copyValue()} /></Scroll>
        <Button label="Done" compact={platform.desktop} onPress={closeInfo} />
      </Dialog>
    </If>
    <If when={editing()}>
      <Dialog title={`Edit ${column()?.name || "cell"}`} onClose={cancelEdit} footer={<View class="flex-row justify-end gap-2">
        <Button label="Cancel" quiet onPress={cancelEdit} /><Button label="Save cell" onPress={() => commitEdit()} />
      </View>}>
        <Text class="text-xs text-muted-foreground">Changed values are saved as text. Unchanged numbers and booleans keep their type. Clear cell saves empty text.</Text>
        <EditInput value={editDraft()} onChange={value => editDraft.set(value)} onCommit={commitEdit} onCancel={cancelEdit} onBind={blur => { blurEditor = blur; }} />
        <If when={operationError()}><Text class="text-xs text-destructive">{operationError()}</Text></If>
      </Dialog>
    </If>
    <If when={full() != null}>
      <ValueDialog text={full() ?? ""} byteLength={selection()?.preview.byteLength ?? 0} copyLabel={copyLabel()}
        title={`Row ${(selection()?.rowIndex ?? 0) + 1} · ${column()?.name || "(unnamed)"}`}
        onCopy={() => void copyValue()} onClose={() => full.set(null)} />
    </If>
    <Menu open={menuPoint() != null} onOpenChange={e => { if (!e.open) menuPoint.set(null); }}
      popupClass={menuPopup} backdropClass="bg-foreground/20"
      trigger={<View accessibilityElementsHidden style={{ position: "absolute", left: menuPoint()?.x ?? 0, top: menuPoint()?.y ?? 0, width: 1, height: 1 }} />}>
      <MenuRow label="Edit cell" disabled={!selection()} onSelect={beginEdit} />
      <MenuSeparator class="my-1 h-px bg-border" />
      <MenuRow label="Clear cell" disabled={!selection()} onSelect={clearCell} />
    </Menu>
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
    <Each of={["CSV", "TSV", "JSON", "NDJSON", "XML"]}>{label => <View class="rounded-full border border-border/60 bg-background px-2.5 py-1 shadow-sm">
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
            <Text class="text-[13px] text-muted-foreground">CSV · TSV · JSON · NDJSON · XML</Text>
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
              <Text class="text-sm text-muted-foreground">Import and inspect structured data.</Text>
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
  // That bar is a DOM node with a huge z-index, and the document view is
  // painted after Diagnostics, so the bar shows through the panel. Isolate
  // the grid and the inspector scroller so the bar stays above their content
  // only, and lift Diagnostics above the document. Hide only the disabled
  // inner indicator beneath the grid.
  cleanup(BrowserStyles.install(
    '[data-testid="data-grid"], [data-testid="inspector-scroll"] { isolation: isolate; }' +
    '[data-testid="diagnostics"] { z-index: 20; }' +
    '[data-testid="data-grid"] .nsh-noindicator > .nsh-bar { display: none; }',
  ));

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
  const format = signal("csv");
  const structuredSize = signal<ImportSize | null>(null);
  const prepared = signal<OpenDocument | null>(null);
  const delimited = () => format() === "csv" || format() === "tsv";
  function discardPrepared() { prepared()?.dispose(); prepared.set(null); structuredSize.set(null); }
  function chooseFormat(value: string) {
    if (format() === value && prepared()) return;
    discardPrepared();
    format.set(value); error.set("");
    if (value === "csv" || value === "tsv") { delimiter.set(value === "tsv" ? "\t" : ","); return; }
    try { structuredSize.set(draft()?.inspectFormat(value) ?? null); } catch (cause) { fail(cause); }
  }
  const analysis = signal<ImportAnalysis | null>(null);
  const error = signal("");
  const busy = signal(false);
  const dragging = signal(false);
  let alive = true;
  let request = 0;
  cleanup(() => { alive = false; request++; BrowserImport.cancelPending(); prepared()?.dispose(); draft()?.dispose(); active()?.dispose(); });
  keymap.bind("ctrl+shift+d", () => { diagnosticsOn.set(!diagnosticsOn()); measurements.set([]); });

  /** Clears review state; the Rust-held file is released here or by cancelPending. */
  function clearDraft() {
    request++;
    prepared()?.dispose(); prepared.set(null);
    draft()?.dispose();
    draft.set(null); pending.set(null); analysis.set(null); structuredSize.set(null); busy.set(false); error.set(""); accepted.set(false);
  }
  function resetImport() { clearDraft(); BrowserImport.cancelPending(); }
  function dismiss() { resetImport(); mode.set(null); paste.set(""); }
  function message(cause: unknown) { return cause instanceof Error ? cause.message : String(cause); }
  function fail(value: unknown) { error.set(message(value)); }
  function inspect(source: ImportSource, retry: "paste" | null) {
    try {
      const result = timed("Inspect", () => source.inspect(), () => bytes(source.byteLength));
      draft.set(source); analysis.set(result); delimiter.set(result.delimiter); headers.set(true); mode.set("review"); chooseFormat(result.format);
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
    const options = { format: format(), delimiter: delimiter(), headers: headers() };
    busy.set(true); error.set("");
    // Let the loading state paint before the synchronous Rust parser runs.
    await new Promise(resolve => setTimeout(resolve, 30));
    if (!alive || id !== request) return;
    let candidate: DocumentSession | null = null;
    try {
      let document = prepared();
      if (!document) {
        candidate = timed("Parse", () => DocumentSession.fromSource(source, options), () => bytes(source.byteLength));
        const session = candidate;
        const metadata = session.metadata();
        let live = true;
        document = { session, metadata, columns: [], name: source.name,
          dispose: () => { if (live) { live = false; session.dispose(); } } };
        prepared.set(document); candidate = null;
        if (!delimited() || !size()?.dimensionsKnown) {
          const exact = session.importSize(source.byteLength);
          structuredSize.set(exact);
          if (exact.large && !accepted()) return;
        }
      }
      document.columns = document.session.columns();
      prepared.set(null);
      active.set(document);
      timeToNextFrame("Document open to next frame", `${document.metadata.columnCount} columns`);
      // The session holds its own data; release the source bytes now.
      source.dispose(); draft.set(null); analysis.set(null); paste.set(""); mode.set(null);
    } catch (cause) { candidate?.dispose(); fail(cause); }
    finally { busy.set(false); }
  }

  const compact = platform.desktop;
  const size = () => { const result = analysis(); return structuredSize() ?? (result ? (delimiter() === "\t" ? result.tab : result.comma) : null); };
  const summary = () => {
    const result = analysis(), measured = size();
    if (!result || !measured) return "";
    if (!measured.dimensionsKnown) return `${bytes(result.byteLength)} · Row and column counts are checked when you import.`;
    const rows = Math.max(0, measured.records - (delimited() && !prepared() && headers() ? 1 : 0));
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
        description={mode() === "paste" ? "Paste CSV, TSV, JSON, NDJSON or XML below. You can review the format before opening it."
          : mode() === "confirm" ? `This file is ${bytes(pending()?.size ?? 0)}. Rowtation has not read it yet.`
          : analysis()?.ambiguous ? "Confirm the format below; detection is uncertain." : "Format detected. Review the options before importing."}
        onClose={dismiss} footer={
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
        }>
        <If when={mode() === "paste"}>
          <Input label="Data to import" testID="paste-input" value={paste()} onChange={value => paste.set(value)} multiline placeholder={"name,city\nMia,Brisbane"} />
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
          <View class="flex-row flex-wrap gap-2">
            <Each of={["csv", "tsv", "json", "ndjson", "xml"]}>{value =>
              <Button testID={`import-format-${value()}`} label={value().toUpperCase()} accessibilityLabel={`${value().toUpperCase()}${format() === value() ? ", selected" : ""}`} quiet={format() !== value()} compact disabled={busy()} onPress={() => chooseFormat(value())} />
            }</Each>
          </View>
          <If when={delimited()}>
          <ChoiceGroup>
            <Choice mark="CSV" label="Comma" checked={delimiter() === ","} disabled={busy()} onSelect={() => chooseFormat("csv")} />
            <Choice mark="TSV" label="Tab" checked={delimiter() === "\t"} disabled={busy()} onSelect={() => chooseFormat("tsv")} />
          </ChoiceGroup>
          <View class="rounded-xl border border-border/70 px-4">
            <Checkbox label="First row contains column headers" checked={headers()} disabled={busy()} onChange={value => { discardPrepared(); headers.set(value); }} />
          </View>
          </If>
          <View class="gap-1">
            <Text testID="import-summary" class="text-[13px] text-foreground">{summary()}</Text>
            <Text class="text-xs text-muted-foreground">{delimited() ? "Values are preserved as text, including leading zeros and formula-like content." : format() === "xml" ? "Attributes use @name; text uses #text. Repeated children use indexed paths. XML values stay text; mixed content and DTDs are rejected." : "Nested paths become columns. Numbers keep exact precision; strings keep their type. NDJSON skips blank lines."}</Text>
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
          <Text class="text-xs text-muted-foreground">Correct the input or choose another format or file. Your current document is unchanged.</Text></View></If>
      </Dialog>}
    </Each>
    <If when={diagnosticsOn()}><DiagnosticsPanel /></If>
  </View>;
}

mount(App);
