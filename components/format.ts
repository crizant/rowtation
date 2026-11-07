/** Display helpers for counts and sizes reported by Rust; no data logic. */
export function count(value: number, one: string, many: string): string {
  return `${value.toLocaleString()} ${value === 1 ? one : many}`;
}

export function bytes(value: number): string {
  if (value < 1024) return count(value, "byte", "bytes");
  const units = ["KB", "MB", "GB"];
  let size = value / 1024;
  let unit = 0;
  while (size >= 1024 && unit < units.length - 1) { size /= 1024; unit++; }
  return `${size.toFixed(size < 10 ? 1 : 0)} ${units[unit]}`;
}
