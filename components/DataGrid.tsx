import { computed, platform, signal, Text, View, windowSize } from "nsh";
import { DataTable, type DataTableRange, type DataTableSelection } from "nsh/base";
import type { CellSnapshot, ColumnMetadata, DocumentMetadata, DocumentSession, RowsSnapshot } from "rowtation/native";

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
  return cell.text ?? cell.number ?? String(cell.boolean ?? "");
}

/** Virtualized NSH table backed exclusively by Rust row windows of at most 256 rows. */
export function DataGrid(props: {
  session: DocumentSession;
  metadata: DocumentMetadata;
  columns: ColumnMetadata[];
  onSelect: (selection: CellSelection | null, column: number | null) => void;
}) {
  const rows = signal<RowsSnapshot>({ documentId: props.metadata.id, revision: 0, offset: 0, rows: [] });
  function load(range: DataTableRange) {
    const start = Math.max(0, range.start - 4);
    const end = Math.min(props.metadata.rowCount, range.end + 4);
    if (rows().offset === start && rows().rows.length === end - start) return;
    const snapshots: RowsSnapshot["rows"] = [];
    for (let offset = start; offset < end; offset += 256) {
      snapshots.push(...props.session.rows({ offset, limit: Math.min(256, end - offset) }).rows);
    }
    rows.set({ documentId: props.metadata.id, revision: props.metadata.revision, offset: start, rows: snapshots });
  }
  load({ start: 0, end: Math.min(32, props.metadata.rowCount) });
  const data = computed(() => rows().rows.map(row => row.cells.map(cellText)));
  const selection = signal<DataTableSelection | null>(null);
  function select(next: DataTableSelection | null) {
    selection.set(next);
    if (next?.column == null) { props.onSelect(null, null); return; }
    const column = props.columns[next.column];
    if (next.row == null) { props.onSelect(null, column.id); return; }
    // Keyboard jumps may precede the viewport event. Fetch only that bounded row.
    const row = rows().rows[next.row - rows().offset]
      ?? props.session.rows({ offset: next.row, limit: 1 }).rows[0];
    if (!row) return;
    props.onSelect({ documentId: props.metadata.id, rowId: row.id, columnId: column.id,
      rowIndex: next.row, columnIndex: next.column, preview: row.cells[next.column] }, column.id);
  }
  // Pointer desktops get a denser grid; touch layouts keep 44-point rows.
  const dense = () => platform.desktop && windowSize().width >= 900;
  return <DataTable class="flex-1 min-h-0 min-w-0 bg-background" testID="data-grid" accessibilityLabel="Document data"
    columns={props.columns.map((column) => ({ key: String(column.id), title: column.name || "(unnamed)", width: windowSize().width < 640 ? 148 : 176 }))}
    data={data()} dataStart={rows().offset} rowCount={props.metadata.rowCount}
    rowHeight={dense() ? 34 : 44} headerHeight={dense() ? 36 : 44} fontSize={13} rowHeader cellSelectable columnSelectable rowSelectable={false}
    movableColumns={false} resizableColumns={false} selectOnModifierPress={false} stripe={false} selection={selection()}
    onVisibleRowsChange={e => load(e.range)} onSelectionChange={e => select(e.selection)}
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
