import { expect, test } from "nsh/test";
import type { DocumentMetadata, RowWindow, RowsSnapshot } from "rowtation/native";
import { loadGridRows, PREVIEW_BYTES } from "./gridRowCache";

function fixture(rowCount = 2000) {
  const metadata: DocumentMetadata = { id: "synthetic", revision: 0, rowCount, columnCount: 40, dirty: false, delimiter: ",", headers: true };
  const columns = Array.from({ length: 40 }, (_, id) => ({ id, name: `Column ${id}` }));
  const calls: RowWindow[] = [];
  const fetch = (window: RowWindow): RowsSnapshot => {
    calls.push(window);
    return { documentId: metadata.id, revision: metadata.revision, offset: window.offset, columnOffset: window.columnOffset ?? 0,
      rows: Array.from({ length: window.limit }, (_, i) => ({ id: window.offset + i,
        cells: Array.from({ length: window.columnLimit ?? 40 }, (_, column) => ({ kind: "text", byteLength: 8, truncated: false, text: `r${window.offset + i}c${(window.columnOffset ?? 0) + column}` })) })) };
  };
  const load = (previous: ReturnType<typeof loadGridRows> | null, start: number, end: number, columnStart = 0, columnEnd = 4) =>
    loadGridRows(previous, metadata, columns, { start, end }, { start: columnStart, end: columnEnd }, fetch);
  return { load, calls, metadata };
}

test("grid cache reuses snapshots and formatted rows without fetching inside its margin", () => {
  const { load, calls } = fixture();
  const initial = load(null, 0, 24);
  const next = load(initial, 20, 44);
  expect(calls.length).toBe(1);
  expect(initial.data.length).toBe(256);
  expect(next).toBe(initial);
  expect(next.snapshot.rows[20]).toBe(initial.snapshot.rows[20]);
  expect(next.data[20]).toBe(initial.data[20]);
  expect(calls[0].previewBytes).toBe(PREVIEW_BYTES);
});

test("grid cache fetches only missing suffix/prefix near either boundary and preserves overlap", () => {
  const { load, calls } = fixture();
  const initial = load(null, 0, 24);
  const down = load(initial, 201, 225);
  expect(calls.length).toBe(2);
  expect(calls[1].offset).toBe(256);
  expect(calls[1].limit).toBe(down.snapshot.offset);
  expect(down.data.length).toBe(256);
  expect(down.snapshot.rows[0]).toBe(initial.snapshot.rows[down.snapshot.offset]);
  expect(down.data[0]).toBe(initial.data[down.snapshot.offset]);
  const stable = load(down, 205, 229);
  expect(stable).toBe(down);
  expect(calls.length).toBe(2);
  const up = load(down, down.snapshot.offset + 31, down.snapshot.offset + 55);
  expect(calls.length).toBe(3);
  expect(calls[2].offset).toBe(up.snapshot.offset);
  expect(calls[2].limit).toBe(down.snapshot.offset - up.snapshot.offset);
  const overlap = down.snapshot.offset - up.snapshot.offset;
  expect(up.snapshot.rows[overlap]).toBe(down.snapshot.rows[0]);
  expect(up.data[overlap]).toBe(down.data[0]);
  expect(up.data.length).toBe(256);
});

test("grid cache fetches a missing suffix when the viewport crosses the cached range", () => {
  const { load, calls } = fixture();
  const initial = load(null, 0, 24);
  const crossed = load(initial, 250, 274);
  expect(calls.length).toBe(2);
  expect(calls[1].offset).toBe(256);
  expect(crossed.data[250 - crossed.snapshot.offset]).toBe(initial.data[250]);
  expect(crossed.data[273 - crossed.snapshot.offset]["0"]).toBe("r273c0");
  expect(crossed.data.length).toBe(256);
});

test("grid cache handles large jumps and document ends with one bounded fetch", () => {
  const { load, calls } = fixture();
  const initial = load(null, 0, 24);
  const jumped = load(initial, 1500, 1524);
  expect(calls.length).toBe(2);
  expect(calls[1].offset).toBe(jumped.snapshot.offset);
  expect(calls[1].limit).toBe(256);
  expect(jumped.data[1500 - jumped.snapshot.offset]["0"]).toBe("r1500c0");
  const bottom = load(jumped, 1976, 2000);
  expect(bottom.snapshot.offset).toBe(1744);
  expect(bottom.data.length).toBe(256);
  expect(bottom.data[255]["0"]).toBe("r1999c0");
  expect(load(bottom, 1975, 1999)).toBe(bottom);
  const top = load(bottom, 0, 24);
  expect(calls.length).toBe(4);
  expect(top.snapshot.offset).toBe(0);
  expect(top.data[0]["0"]).toBe("r0c0");
});

test("grid cache invalidates column windows, document identity and revision", () => {
  const { load, calls, metadata } = fixture();
  const initial = load(null, 0, 24);
  expect(load(initial, 0, 24, 2, 6)).toBe(initial);
  const horizontal = load(initial, 0, 24, 20, 24);
  expect(calls.length).toBe(2);
  expect(horizontal.snapshot.columnOffset).toBe(16);
  expect(horizontal.data[0]["20"]).toBe("r0c20");
  expect(horizontal.data[0]).not.toBe(initial.data[0]);
  metadata.revision++;
  const revised = load(horizontal, 0, 24, 20, 24);
  expect(calls.length).toBe(3);
  expect(revised.data[0]).not.toBe(horizontal.data[0]);
  metadata.id = "replacement";
  const replacement = load(revised, 0, 24, 20, 24);
  expect(calls.length).toBe(4);
  expect(replacement.snapshot.documentId).toBe("replacement");
});

test("grid cache ignores transient empty viewport ranges", () => {
  const { load, calls } = fixture();
  const initial = load(null, 0, 24);
  expect(load(initial, 0, 24, 40, 40)).toBe(initial);
  expect(load(initial, 2000, 2000)).toBe(initial);
  expect(calls.length).toBe(1);
  const visible = load(initial, 1500, 1524);
  expect(calls.length).toBe(2);
  expect(visible.data[1500 - visible.snapshot.offset]["0"]).toBe("r1500c0");
});

test("grid cache handles small and empty documents without repeated fetches", () => {
  const small = fixture(60);
  const initial = small.load(null, 0, 24);
  expect(initial.data.length).toBe(60);
  expect(small.load(initial, 36, 60)).toBe(initial);
  expect(small.calls.length).toBe(1);
  const empty = fixture(0);
  const noRows = empty.load(null, 0, 0);
  expect(noRows.data.length).toBe(0);
  expect(empty.load(noRows, 0, 0)).toBe(noRows);
  expect(empty.calls.length).toBe(0);
});
