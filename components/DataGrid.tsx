import { computed, platform, signal, Text, View, windowSize } from "nsh";
import { DataTable, type DataTableRange, type DataTableSelection } from "nsh/base";
import type { CellSnapshot, ColumnMetadata, DocumentMetadata, DocumentSession, RowsSnapshot, RowWindow } from "rowtation/native";
import { measuring, record, timeToNextFrame } from "./diagnostics";

/** Grid cells show at most this many UTF-8 bytes; the inspector reads whole values on demand. */
export const PREVIEW_BYTES = 512;

export interface CellSelection {
  documentId: string;
  rowId: number;
  columnId: number;
  rowIndex: number;
  columnIndex: number;
  preview: CellSnapshot;
}
export function cellText(cell: CellSnapshot): string {
  if (cell.kind === "missing") return "—";
  if (cell.kind === "null") return "null";
  const text = cell.text ?? cell.number ?? String(cell.boolean ?? "");
  return cell.truncated ? `${text}…` : text;
}

/** Virtualized NSH table backed by Rust windows of the rows and columns in view, with previews of long values. */
export function DataGrid(props: {
  session: DocumentSession;
  metadata: DocumentMetadata;
  columns: ColumnMetadata[];
  onSelect: (selection: CellSelection | null, column: number | null) => void;
}) {
  const rows = signal<RowsSnapshot>({ documentId: props.metadata.id, revision: 0, offset: 0, columnOffset: 0, rows: [] });
  let visibleRows: DataTableRange = { start: 0, end: Math.min(32, props.metadata.rowCount) };
  let visibleColumns: DataTableRange = { start: 0, end: Math.min(16, props.metadata.columnCount) };
  let loadedColumns: DataTableRange = { start: 0, end: 0 };
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
  function load() {
    const start = Math.max(0, visibleRows.start - 4);
    const end = Math.min(props.metadata.rowCount, visibleRows.end + 4);
    const columns = visibleColumns.start >= loadedColumns.start && visibleColumns.end <= loadedColumns.end ? loadedColumns
      : { start: Math.max(0, visibleColumns.start - 4), end: Math.min(props.metadata.columnCount, visibleColumns.end + 4) };
    if (rows().offset === start && rows().rows.length === end - start && columns === loadedColumns) return;
    const snapshots: RowsSnapshot["rows"] = [];
    for (let offset = start; offset < end; offset += 256) {
      snapshots.push(...fetch({ offset, limit: Math.min(256, end - offset), columnOffset: columns.start,
        columnLimit: columns.end - columns.start, previewBytes: PREVIEW_BYTES }).rows);
    }
    loadedColumns = columns;
    rows.set({ documentId: props.metadata.id, revision: props.metadata.revision, offset: start, columnOffset: columns.start, rows: snapshots });
    timeToNextFrame("Grid update to next frame", `${snapshots.length} rows × ${columns.end - columns.start} columns`);
  }
  load();
  // Objects keyed by column ID hold only the loaded columns; the table shows others empty until they load.
  const data = computed(() => {
    const keys = props.columns.slice(rows().columnOffset, loadedColumns.end).map(column => String(column.id));
    return rows().rows.map(row => {
      const cells: Record<string, string> = {};
      row.cells.forEach((cell, i) => { cells[keys[i]] = cellText(cell); });
      return cells;
    });
  });
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
  // Pointer desktops get a denser grid; touch layouts keep 44-point rows.
  const dense = () => platform.desktop && windowSize().width >= 900;
  return <DataTable class="flex-1 min-h-0 min-w-0 bg-background" testID="data-grid" accessibilityLabel="Document data"
    columns={props.columns.map((column) => ({ key: String(column.id), title: column.name || "(unnamed)", width: windowSize().width < 640 ? 148 : 176 }))}
    data={data()} dataStart={rows().offset} rowCount={props.metadata.rowCount}
    rowHeight={dense() ? 34 : 44} headerHeight={dense() ? 36 : 44} fontSize={13} rowHeader cellSelectable columnSelectable rowSelectable={false}
    movableColumns={false} resizableColumns={false} selectOnModifierPress={false} stripe={false} selection={selection()}
    onVisibleRowsChange={e => { visibleRows = e.range; load(); }}
    onVisibleColumnsChange={e => { visibleColumns = e.range; load(); }} onSelectionChange={e => select(e.selection)}
    renderHeaderCell={cell => <View class="flex-1 flex-row items-center gap-2 px-3">
      <View class="h-4 min-w-5 items-center justify-center rounded bg-muted px-1"><Text class="text-[10px] font-medium text-muted-foreground">Aa</Text></View>
      <Text class="line-clamp-1 flex-1 text-xs font-medium text-muted-foreground">{cell.title}</Text>
    </View>}
    classes={{ root: "border-0", header: "bg-background border-b border-border/70", headerCell: "border-0",
      row: "border-b border-border/50", rowHover: "bg-muted/40", cell: "px-3 text-foreground",
      cellSelected: "bg-background border-2 border-foreground rounded-[3px]", cellColumnSelected: "bg-muted/50",
      rowHeader: "text-muted-foreground bg-background text-xs", rowHeaderCorner: "bg-background" }}
    empty={<View class="flex-1 items-center justify-center gap-2 p-6"><Text class="text-foreground">No data rows</Text>
      <Text class="text-sm text-muted-foreground">This document is empty or contains only headers.</Text></View>} />;
}
