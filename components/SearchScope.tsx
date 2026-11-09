import { Text, View } from "nsh";
import { Select } from "nsh/base";
import type { ColumnMetadata } from "rowtation/native";

/** Column identities stay opaque; this control only presents the Rust schema. */
export function SearchScope(props: { open: boolean; onOpenChange: (open: boolean) => void; compact?: boolean; columns: ColumnMetadata[]; value: number | null; onChange: (value: number | null) => void }) {
  const title = () => {
    if (props.value == null) return "All columns";
    const index = props.columns.findIndex(column => column.id === props.value);
    const column = props.columns[index];
    if (!column) return "Selected column";
    if (!column.name.trim()) return `Column ${index + 1} (unnamed)`;
    // Lead with identity so truncation cannot hide which duplicate is selected.
    return props.columns.some((other, position) => position !== index && other.name === column.name)
      ? `Column ${index + 1} · ${column.name}` : column.name;
  };
  return <View class="min-w-0 flex-row items-center gap-3">
    <Text class="shrink-0 text-xs text-muted-foreground">Search in</Text>
    <Select open={props.open} onOpenChange={event => props.onOpenChange(event.open)} class="min-w-0 flex-1" accessibilityLabel="Search in"
      items={[{ value: "all", title: "All columns" }, ...props.columns.map((column, index) => ({ value: String(column.id), title: `${column.name.trim() ? column.name : "Unnamed column"} · Column ${index + 1}` }))]}
      value={props.value == null ? "all" : String(props.value)} onChange={event => props.onChange(event.value == null || event.value === "all" ? null : Number(event.value))}
      popupClass="rounded-2xl border border-border bg-background p-2 shadow-lg" backdropClass="bg-foreground/20"
      listClass="min-h-0" itemClass="min-h-11 px-3 py-2 rounded-lg text-sm text-foreground focus:bg-muted selected:bg-muted cursor-pointer"
      renderItem={item => <Text class="text-sm text-foreground">{item.title}</Text>}
      trigger={<View testID="search-scope" class={"min-w-0 flex-row items-center gap-2 rounded-xl border border-border bg-background px-3 cursor-pointer focus:ring-2 focus:ring-ring " + (props.compact ? "min-h-9" : "min-h-11")}>
        <Text class="min-w-0 flex-1 line-clamp-1 text-sm text-foreground">{title()}</Text>
        <Text accessibilityElementsHidden class="shrink-0 text-sm text-muted-foreground">▾</Text>
      </View>} />
  </View>;
}
