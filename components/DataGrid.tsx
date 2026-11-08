import { computed, effect, platform, signal, Text, View, windowSize } from "nsh";
import { DataTable, type DataTableCell, type DataTableRange, type DataTableRef, type DataTableSort, type DataTableSelection } from "nsh/base";
import type { CellSnapshot, ColumnMetadata, DocumentMetadata, DocumentSession, RowWindow } from "rowtation/native";
import { measuring, record, timeToNextFrame } from "./diagnostics";
import { loadGridRows, PREVIEW_BYTES, utf8Span } from "./gridRowCache";
export { cellText, PREVIEW_BYTES } from "./gridRowCache";

export interface CellSelection {
  documentId: string;
  rowId: number;
  columnId: number;
  rowIndex: number;
  columnIndex: number;
  preview: CellSnapshot;
}
/** Virtualized NSH table backed by Rust windows of the rows and columns in view, with previews of long values. */
export function DataGrid(props: {
  session: DocumentSession;
  metadata: DocumentMetadata;
  columns: ColumnMetadata[];
  version: number;
  selected: CellSelection | null;
  sort: DataTableSort | null;
  onSort: (sort: DataTableSort | null) => void;
  onEdit: (rowIndex: number, columnIndex: number) => void;
  onCellMenu: (rowIndex: number, columnIndex: number, x: number, y: number) => void;
  onReady: (focus: () => void) => void;
  onSelect: (selection: CellSelection | null, column: number | null) => void;
  activeMatch?: { row: number; column: number; start: number; end: number } | null;
}) {
  let visibleRows: DataTableRange = { start: 0, end: Math.min(32, props.metadata.rowCount) };
  // Start conservatively until the table reports its measured column range.
  let visibleColumns: DataTableRange = { start: 0, end: Math.min(4, props.metadata.columnCount) };
  function fetch(window: RowWindow) {
    if (!measuring()) return props.session.rows(window);
    const started = performance.now();
    const snapshot = props.session.rows({ ...window, measure: true });
    const total = performance.now() - started;
    // One call: Rust times its own work; the rest of the same call is an estimate of
    // the copy into JS plus call overhead, limited by the browser's timer resolution.
    const rust = snapshot.rustMs ?? 0;
    record("Row window", total, `${snapshot.rows.length} rows × ${window.columnLimit ?? props.metadata.columnCount} columns; Rust ${rust.toFixed(2)} ms measured, transfer ≈${Math.max(0, total - rust).toFixed(2)} ms estimated`);
    return snapshot;
  }
  // This cache lives with the grid; document/revision and column changes invalidate reuse.
  const cache = signal(loadGridRows(null, props.metadata, props.columns, visibleRows, visibleColumns, fetch));
  const rows = () => cache().snapshot;
  function load() {
    const previous = cache();
    const next = loadGridRows(previous, props.metadata, props.columns, visibleRows, visibleColumns, fetch);
    if (next === previous) return;
    cache.set(next);
    timeToNextFrame("Grid update to next frame", `${next.data.length} rows × ${next.columnEnd - next.snapshot.columnOffset} columns`);
  }
  const selection = signal<DataTableSelection | null>(null);
  function select(next: DataTableSelection | null) {
    selection.set(next);
    if (next?.column == null) { props.onSelect(null, null); return; }
    const column = props.columns[next.column];
    if (next.row == null) { props.onSelect(null, column.id); return; }
    // Keyboard jumps may precede the viewport events. Fetch only that cell's preview.
    const loaded = rows().rows[next.row - rows().offset];
    const cell = loaded?.cells[next.column - rows().columnOffset];
    const row = cell ? loaded : props.session.rows({ offset: next.row, limit: 1, columnOffset: next.column, columnLimit: 1, previewBytes: PREVIEW_BYTES }).rows[0];
    if (!row) return;
    props.onSelect({ documentId: props.metadata.id, rowId: row.id, columnId: column.id,
      rowIndex: next.row, columnIndex: next.column, preview: cell ?? row.cells[0] }, column.id);
  }
  let table: DataTableRef | undefined;
  props.onReady(() => table?.focus());
  const active = signal(props.activeMatch ?? null);
  effect(() => { active.set(props.activeMatch ?? null); });
  effect(() => {
    props.version;
    cache.set(loadGridRows(null, props.metadata, props.columns, visibleRows, visibleColumns, fetch));
  });
  effect(() => {
    const cell = props.selected;
    const next = cell ? { row: cell.rowIndex, column: cell.columnIndex } : null;
    selection.set(next);
    table?.select(next);
  });
  // Pointer desktops get a denser grid; touch layouts keep 44-point rows.
  const dense = () => platform.desktop && windowSize().width >= 900;
  const columnWidth = () => windowSize().width < 640 ? 148 : 176;
  // Keep a stable renderer identity: a row-window refill must not reset the
  // table's renderCell prop. Ordinary pointer cells use NSH's native text path.
  // Touch cells retain the lightweight wrapper for NSH 0.4's onLongPress event.
  const highlights = computed(() => cache().snapshot.rows.some(row => row.cells.some(cell => cell.matched)));
  const renderCell = (cell: DataTableCell) => <GridCell text={cell.text} row={cell.row} column={cell.column} />;
  function GridCell(cell: { text: string; row: number; column: number }) {
    // Overlapping cache windows reuse snapshots. Only this memo subscribes to
    // the whole cache; unchanged cells do not reapply text, spans or styles.
    const snapshot = computed(() => {
      const current = cache().snapshot;
      return current.rows[cell.row - current.offset]?.cells[cell.column - current.columnOffset];
    });
    const matched = computed(() => !!snapshot()?.matched);
    const occurrence = computed(() => {
      const mark = active();
      if (!mark || mark.row !== cell.row || mark.column !== cell.column) return null;
      const snap = snapshot();
      const split = utf8Span(snap?.text ?? snap?.number ?? (snap?.boolean == null ? "" : String(snap.boolean)), mark.start, mark.end);
      return split?.hit ? { ...split, after: split.after + (snap?.truncated ? "…" : "") } : null;
    });
    return <View testID={matched() ? "search-highlight" : "grid-custom-cell"}
      onLongPress={() => props.onCellMenu(cell.row, cell.column, 16, 120)}
      style={{ height: dense() ? 30 : 40, width: columnWidth() - 24 }}
      class={"justify-center rounded" + (matched() ? " bg-primary/15" : "")}>
      <If when={occurrence()} else={<Text class="text-[13px] text-foreground line-clamp-1">{cell.text}</Text>}>
        {split => <Text class="text-[13px] text-foreground line-clamp-1">{split().before}<Text
          testID="active-match" class="rounded-sm bg-primary/50 text-foreground underline"
          style={{ textDecorationLine: "underline" }}>{split().hit}</Text>{split().after}</Text>}
      </If>
    </View>;
  }
  return <DataTable renderCell={highlights() || !platform.desktop ? renderCell : undefined}
    class="flex-1 min-h-0 min-w-0 bg-background" testID="data-grid" accessibilityLabel="Document data"
    ref={ref => { table = ref; table.select(selection()); }} sort={props.sort} onSortChange={e => props.onSort(e.sort)}
    onDoublePress={e => { if (e.target.row != null && e.target.column != null) props.onEdit(e.target.row, e.target.column); }}
    onContextMenu={e => { if (e.row != null && e.column != null) props.onCellMenu(e.row, e.column, e.x, e.y); }}
    columns={props.columns.map((column) => ({ key: String(column.id), sortable: true, title: column.name || "(unnamed)", width: columnWidth() }))}
    data={cache().data} dataStart={rows().offset} rowCount={props.metadata.rowCount}
    rowHeight={dense() ? 34 : 44} headerHeight={dense() ? 36 : 44} fontSize={13} rowHeader cellSelectable columnSelectable rowSelectable={false}
    movableColumns={false} resizableColumns={false} selectOnModifierPress={false} stripe={false} selection={selection()}
    onVisibleRowsChange={e => { visibleRows = e.range; load(); }}
    onVisibleColumnsChange={e => { visibleColumns = e.range; load(); }} onSelectionChange={e => select(e.selection)}
    renderHeaderCell={cell => <View class="flex-1 flex-row items-center gap-2 px-3">
      <If when={props.metadata.format !== "json" && props.metadata.format !== "ndjson"}><View class="h-4 min-w-5 items-center justify-center rounded bg-muted px-1"><Text class="text-[10px] font-medium text-muted-foreground">Aa</Text></View></If>
      <Text class="line-clamp-1 flex-1 text-xs font-medium text-muted-foreground">{cell.title}</Text>
    </View>}
    classes={{ root: "border-0", header: "bg-background border-b border-border/70", headerCell: "border-0",
      row: "border-b border-border/50", rowHover: "bg-muted/40", cell: "px-3 text-foreground",
      cellSelected: "bg-background border-2 border-foreground rounded-[3px]", cellColumnSelected: "bg-muted/50",
      sortIndicator: "text-muted-foreground", sortAscending: "text-muted-foreground selected:text-foreground",
      sortDescending: "text-muted-foreground selected:text-foreground",
      rowHeader: "text-muted-foreground bg-background text-xs", rowHeaderCorner: "bg-background" }}
    empty={<View class="flex-1 items-center justify-center gap-2 p-6"><Text class="text-foreground">{props.metadata.columnCount === 0 && props.metadata.rowCount > 0 ? "No fields to display" : "No data rows"}</Text>
      <Text class="text-sm text-muted-foreground">{props.metadata.format === "json" || props.metadata.format === "ndjson" ? "Empty objects and arrays are retained in the source structure." : "This document is empty or contains only headers."}</Text></View>} />;
}
