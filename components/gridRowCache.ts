import type { DataTableRange } from "nsh/base";
import type { CellSnapshot, ColumnMetadata, DocumentMetadata, RowsSnapshot, RowWindow } from "rowtation/native";

/** Grid cells show at most this many UTF-8 bytes; the inspector reads whole values on demand. */
export const PREVIEW_BYTES = 512;

function utf8Size(codePoint: number) {
  if (codePoint <= 0x7f) return 1;
  if (codePoint <= 0x7ff) return 2;
  if (codePoint <= 0xffff) return 3;
  return 4;
}

/** The longest prefix of at most `limit` UTF-8 bytes that ends on a character boundary. */
export function utf8Prefix(text: string, limit = PREVIEW_BYTES) {
  let bytes = 0;
  let index = 0;
  while (index < text.length) {
    const codePoint = text.codePointAt(index)!;
    const size = utf8Size(codePoint);
    if (bytes + size > limit) break;
    bytes += size;
    index += codePoint > 0xffff ? 2 : 1;
  }
  return text.slice(0, index);
}

/** Split `text` on a UTF-8 byte range. Null when the range is outside the string or splits a character. */
export function utf8Span(text: string, start: number, end: number) {
  if (end < start || start < 0) return null;
  let bytes = 0;
  let index = 0;
  let startIndex = -1;
  while (index < text.length) {
    if (bytes === start) startIndex = index;
    else if (startIndex < 0 && bytes > start) return null;
    const codePoint = text.codePointAt(index)!;
    const next = index + (codePoint > 0xffff ? 2 : 1);
    bytes += utf8Size(codePoint);
    if (startIndex >= 0 && bytes === end) return { before: text.slice(0, startIndex), hit: text.slice(startIndex, next), after: text.slice(next) };
    if (startIndex >= 0 && bytes > end) return null;
    index = next;
  }
  if (startIndex >= 0 && bytes === end) return { before: text.slice(0, startIndex), hit: text.slice(startIndex), after: "" };
  if (bytes === start && start === end) return { before: text, hit: "", after: "" };
  return null;
}

/** Whether a match's byte range falls entirely inside the grid preview of `text`. */
export function previewHolds(text: string, start: number, end: number) {
  return utf8Span(utf8Prefix(text), start, end) != null;
}

export function cellText(cell: CellSnapshot): string {
  if (cell.kind === "missing") return "—";
  if (cell.kind === "null") return "null";
  const text = cell.text ?? cell.number ?? String(cell.boolean ?? "");
  return cell.truncated ? `${text}…` : text;
}

interface GridRowCache {
  snapshot: RowsSnapshot;
  columnEnd: number;
  data: Record<string, string>[];
}

/** Presentation-only cache; Rust remains the owner of document data and revisions. */
export function loadGridRows(
  previous: GridRowCache | null,
  metadata: DocumentMetadata,
  columns: ColumnMetadata[],
  visibleRows: DataTableRange,
  visibleColumns: DataTableRange,
  fetch: (window: RowWindow) => RowsSnapshot,
): GridRowCache {
  const valid = previous?.snapshot.documentId === metadata.id && previous.snapshot.revision === metadata.revision;
  // Layout can briefly report an empty range while a table window changes.
  // Keep the current cache until there is a real viewport to satisfy.
  if (valid && (visibleRows.end <= visibleRows.start || visibleColumns.end <= visibleColumns.start)) return previous;
  const sameColumns = valid && visibleColumns.start >= previous.snapshot.columnOffset && visibleColumns.end <= previous.columnEnd;
  const columnStart = sameColumns ? previous.snapshot.columnOffset : Math.max(0, visibleColumns.start - 4);
  const columnEnd = sameColumns ? previous.columnEnd : Math.min(metadata.columnCount, visibleColumns.end + 4);
  // Refill before an edge is reached, then recenter to leave room in both directions.
  const size = Math.min(256, metadata.rowCount);
  const margin = Math.min(32, Math.max(0, Math.floor((size - (visibleRows.end - visibleRows.start)) / 4)));
  const oldStart = previous?.snapshot.offset ?? 0;
  const oldEnd = oldStart + (previous?.snapshot.rows.length ?? 0);
  if (sameColumns && visibleRows.start >= oldStart && visibleRows.end <= oldEnd
    && (oldStart === 0 || visibleRows.start >= oldStart + margin)
    && (oldEnd === metadata.rowCount || visibleRows.end <= oldEnd - margin)) return previous;

  const start = Math.max(0, Math.min(metadata.rowCount - size,
    visibleRows.start - Math.max(0, Math.floor((size - (visibleRows.end - visibleRows.start)) / 2))));
  const end = start + size;
  const snapshots: RowsSnapshot["rows"] = [];
  const data: Record<string, string>[] = [];
  const keys = columns.slice(columnStart, columnEnd).map(column => String(column.id));
  for (let offset = start; offset < end;) {
    if (sameColumns && offset >= oldStart && offset < oldEnd) {
      const index = offset - oldStart;
      snapshots.push(previous.snapshot.rows[index]);
      data.push(previous.data[index]);
      offset++;
    } else {
      // A shift needs only a prefix and/or suffix; a distant jump needs one window.
      const limit = (sameColumns && offset < oldStart ? Math.min(end, oldStart) : end) - offset;
      const fetched = fetch({ offset, limit, columnOffset: columnStart, columnLimit: columnEnd - columnStart, previewBytes: PREVIEW_BYTES });
      for (const row of fetched.rows) {
        const cells: Record<string, string> = {};
        row.cells.forEach((cell, i) => { cells[keys[i]] = cellText(cell); });
        snapshots.push(row);
        data.push(cells);
      }
      offset += limit;
    }
  }
  return { snapshot: { documentId: metadata.id, revision: metadata.revision, offset: start, columnOffset: columnStart, rows: snapshots }, columnEnd, data };
}

/** Refresh presentation flags only. Keep text/data identity and unchanged cells. */
export function updateGridHighlights(previous: GridRowCache, flags: boolean[][]): GridRowCache {
  let rows: RowsSnapshot["rows"] | null = null;
  previous.snapshot.rows.forEach((row, r) => {
    if (!row.cells.some((cell, c) => cell.matched !== (flags[r]?.[c] ?? false))) return;
    rows ??= previous.snapshot.rows.slice();
    rows[r] = { ...row, cells: row.cells.map((cell, c) => {
      const matched = flags[r]?.[c] ?? false;
      return matched === cell.matched ? cell : { ...cell, matched };
    }) };
  });
  return rows ? { ...previous, snapshot: { ...previous.snapshot, rows } } : previous;
}
