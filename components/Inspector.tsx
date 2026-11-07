import { Text, View } from "nsh";
import type { ColumnMetadata, ColumnProfile, DocumentMetadata } from "rowtation/native";
import type { CellSelection } from "./DataGrid";
import { cellText } from "./DataGrid";

function Stat(props: { label: string; value: string }) {
  return <View class="min-h-11 flex-row items-center justify-between gap-4 border-b border-border/70 py-2">
    <Text class="text-sm text-foreground">{props.label}</Text><Text class="text-sm text-muted-foreground">{props.value}</Text>
  </View>;
}
export function Inspector(props: { column: ColumnMetadata | null; profile: ColumnProfile | null; metadata: DocumentMetadata; selection: CellSelection | null }) {
  return <View class="gap-5 p-5">
    <Text class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">{props.column ? `Column · ${props.column.name || "(unnamed)"}` : "Column information"}</Text>
    <If when={props.profile} else={<Text class="text-sm text-muted-foreground">Select a cell or column header to inspect its values.</Text>}>
      <View>
        <Stat label="Type" value={props.profile?.kind ?? "Text"} />
        <Stat label="Unique values" value={String(props.profile?.uniqueCount ?? 0)} />
        <Stat label="Empty values" value={String(props.profile?.emptyCount ?? 0)} />
        <Stat label="Missing fields" value={String(props.profile?.missingCount ?? 0)} />
      </View>
      <View class="gap-2">
        <Text class="text-xs font-medium uppercase tracking-wider text-muted-foreground">Most common</Text>
        <Each of={props.profile?.commonValues ?? []}>{value => <View class="min-h-11 flex-row items-center gap-3 border-b border-border/70 py-2">
          <Text class="line-clamp-1 flex-1 text-sm text-foreground">{value().value}{value().truncated ? "…" : ""}</Text>
          <Text class="text-sm text-muted-foreground">{value().count}</Text>
        </View>}</Each>
        <If when={props.profile?.commonValues.length === 0}><Text class="text-sm text-muted-foreground">No nonempty values</Text></If>
      </View>
    </If>
    <If when={props.selection}>
      <View class="gap-2"><Text class="text-xs font-medium uppercase tracking-wider text-muted-foreground">Selected value</Text>
        <Text class="line-clamp-5 text-sm text-foreground">{props.selection ? cellText(props.selection.preview) || "(empty text)" : ""}</Text>
        <If when={props.selection?.preview.truncated}><Text class="text-xs text-muted-foreground">Preview · {props.selection?.preview.byteLength} bytes</Text></If>
      </View>
    </If>
    <View class="gap-2">
      <Text class="text-xs font-medium uppercase tracking-wider text-muted-foreground">Source</Text>
      <Stat label="Format" value={props.metadata.delimiter === "\t" ? "TSV · Tab-separated" : "CSV · Comma-separated"} />
      <Stat label="Header row" value={props.metadata.headers ? "Yes" : "No"} />
      <Stat label="Encoding" value="UTF-8" />
    </View>
  </View>;
}
