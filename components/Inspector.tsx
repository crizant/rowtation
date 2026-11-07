import { Text, View } from "nsh";
import type { ColumnMetadata, ColumnProfile, DocumentMetadata } from "rowtation/native";
import type { CellSelection } from "./DataGrid";
import { cellText } from "./DataGrid";
import { bytes } from "./format";

function Stat(props: { label: string; value: string }) {
  return <View class="min-h-9 flex-row items-center justify-between gap-4 border-b border-border/60 py-1.5">
    <Text class="text-[13px] text-foreground">{props.label}</Text><Text class="line-clamp-1 shrink text-[13px] text-muted-foreground">{props.value}</Text>
  </View>;
}
function Section(props: { title: string }) {
  return <Text class="pt-2 text-xs font-medium uppercase tracking-wider text-muted-foreground">{props.title}</Text>;
}
export function Inspector(props: { column: ColumnMetadata | null; profile: ColumnProfile | null; metadata: DocumentMetadata; selection: CellSelection | null }) {
  return <View class="gap-1 p-4">
    <Text class="pb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">{props.column ? `Column · ${props.column.name || "(unnamed)"}` : "Column information"}</Text>
    <If when={props.profile} else={<Text class="pb-2 text-[13px] text-muted-foreground">Select a cell or column header to inspect its values.</Text>}>
      <View>
        <Stat label="Type" value={props.profile?.kind ?? "Text"} />
        <Stat label="Unique values" value={(props.profile?.uniqueCount ?? 0).toLocaleString()} />
        <Stat label="Empty values" value={(props.profile?.emptyCount ?? 0).toLocaleString()} />
        <Stat label="Missing fields" value={(props.profile?.missingCount ?? 0).toLocaleString()} />
      </View>
      <Section title="Most common" />
      <Each of={props.profile?.commonValues ?? []}>{value => <View class="min-h-9 flex-row items-center gap-3 border-b border-border/60 py-1.5">
        <Text class="line-clamp-1 flex-1 text-[13px] text-foreground">{value().value}</Text>
        <Text class="text-[13px] text-muted-foreground">{value().count.toLocaleString()}</Text>
      </View>}</Each>
      <If when={props.profile?.commonValues.length === 0}><Text class="text-[13px] text-muted-foreground">No nonempty values</Text></If>
    </If>
    <If when={props.selection}>
      <Section title="Selected value" />
      <Text class="line-clamp-5 text-[13px] text-foreground">{props.selection ? cellText(props.selection.preview) || "(empty text)" : ""}</Text>
      <Text class="text-xs text-muted-foreground">{bytes(props.selection?.preview.byteLength ?? 0)}</Text>
    </If>
    <Section title="Source" />
    <Stat label="Format" value={props.metadata.delimiter === "\t" ? "TSV · Tab-separated" : "CSV · Comma-separated"} />
    <Stat label="Header row" value={props.metadata.headers ? "Yes" : "No"} />
    <Stat label="Encoding" value="UTF-8" />
  </View>;
}
