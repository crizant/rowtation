import { signal, untracked } from "nsh";
import { Diagnostics } from "rowtation/native";

export interface Measurement { label: string; ms: number; detail: string }

/** Off by default. When on, timings go to this tab's console and panel only. */
export const diagnosticsOn = signal(false);
export const measurements = signal<Measurement[]>([]);

export function measuring() { return untracked(diagnosticsOn); }

export function record(label: string, ms: number, detail = "") {
  if (!measuring()) return;
  console.info(`[rowtation] ${label}: ${ms.toFixed(2)} ms${detail ? ` (${detail})` : ""}`);
  untracked(() => measurements.set([...measurements().slice(-11), { label, ms, detail }]));
}

export function timed<T>(label: string, run: () => T, detail?: () => string): T {
  if (!measuring()) return run();
  const started = performance.now();
  const result = run();
  record(label, performance.now() - started, detail?.() ?? "");
  return result;
}

/** Time from now to the next frame: the NSH update, layout and paint a change caused. */
export function timeFrame(label: string, detail = "") {
  if (!measuring()) return;
  const started = performance.now();
  requestAnimationFrame(() => record(label, performance.now() - started, detail));
}

export function wasmMemory() { return Diagnostics.memory().wasmBytes; }
