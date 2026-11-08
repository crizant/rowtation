import { Text, View } from "nsh";
import type { ColumnMetadata, ColumnProfile, DocumentMetadata } from "rowtation/native";
import type { CellSelection } from "./DataGrid";
import { cellText } from "./DataGrid";
import { bytes } from "./format";
import { Button as BaseButton } from "nsh/base";
import { Icon } from "./Icon";

function Stat(props: { label: string; value: string }) {
  return <View class="min-h-9 flex-row items-center justify-between gap-4 border-b border-border/60 py-1.5">
    <Text class="text-[13px] text-foreground">{props.label}</Text><Text class="line-clamp-1 shrink text-[13px] text-muted-foreground">{props.value}</Text>
  </View>;
}
function Section(props: { title: string }) {
  return <Text class="pt-2 text-xs font-medium uppercase tracking-wider text-muted-foreground">{props.title}</Text>;
}
export function Inspector(props: {
  column: ColumnMetadata | null; profile: ColumnProfile | null; metadata: DocumentMetadata; selection: CellSelection | null; progress: number | null;
  copyLabel: string; onShowValue: () => void; onCopyValue: () => void;
}) {
  const value = () => props.selection?.preview;
  const hasValue = () => value()?.kind === "text" || value()?.kind === "number";
  // Whether the five-line preview above may hide part of the value.
  const clipped = () => {
    const cell = value();
    const text = cell?.text ?? cell?.number ?? "";
    return !!cell?.truncated || text.length > 160 || text.split("\n").length > 5;
  };
  return <View class="gap-1 p-4">
    <Text class="pb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">{props.column ? `Column · ${props.column.name || "(unnamed)"}` : "Column information"}</Text>
    <If when={props.profile} else={<Text class="pb-2 text-[13px] text-muted-foreground">{props.column && props.progress != null
      ? `Counting values… ${Math.floor(props.progress * 100)}%`
      : "Select a cell or column header to inspect its values."}</Text>}>
      <View>
        <Stat label="Type" value={props.profile?.kind ?? "Text"} />
        <Stat label="Unique values" value={(props.profile?.uniqueCount ?? 0).toLocaleString()} />
        <Stat label="Empty values" value={(props.profile?.emptyCount ?? 0).toLocaleString()} />
        <Stat label="Null values" value={(props.profile?.nullCount ?? 0).toLocaleString()} />
        <Stat label="Missing fields" value={(props.profile?.missingCount ?? 0).toLocaleString()} />
      </View>
      <Section title="Most common" />
      <Each of={props.profile?.commonValues ?? []}>{value => <View class="min-h-9 flex-row items-center gap-3 border-b border-border/60 py-1.5">
        <Text class="line-clamp-1 flex-1 text-[13px] text-foreground">{value().truncated ? `${value().value}…` : value().value}</Text>
        <Text class="text-[13px] text-muted-foreground">{value().count.toLocaleString()}</Text>
      </View>}</Each>
      <If when={props.profile?.commonValues.length === 0}><Text class="text-[13px] text-muted-foreground">No nonempty values</Text></If>
    </If>
    <If when={props.selection}>
      <Section title="Selected value" />
      <View class="flex-row items-start gap-2">
        <Text testID="selected-value" class="flex-1 line-clamp-5 text-[13px] leading-5 text-foreground">{value() ? cellText(value()!) || "(empty text)" : ""}</Text>
        <If when={hasValue()}>
          <BaseButton testID="copy-value" accessibilityLabel={props.copyLabel} onPress={() => props.onCopyValue()}
            class="cursor-pointer -mr-1 -mt-1 h-7 w-7 shrink-0 items-center justify-center rounded-md hover:bg-muted active:bg-muted focus:ring-2 focus:ring-ring">
            <Icon name={props.copyLabel === "Copied" ? "check" : "copy"} color="muted-foreground" class="h-4 w-4" />
          </BaseButton>
        </If>
      </View>
      <If when={hasValue()}>
        <View class="min-h-6 flex-row items-center justify-between gap-2">
          <Text class="text-xs text-muted-foreground">{value()?.truncated ? `${bytes(value()?.byteLength ?? 0)} · preview` : bytes(value()?.byteLength ?? 0)}</Text>
          <If when={clipped()}>
            <BaseButton onPress={() => props.onShowValue()} class="cursor-pointer rounded px-1 -mr-1 hover:bg-muted focus:ring-2 focus:ring-ring">
              <Text class="text-xs font-medium text-foreground">View full value</Text>
            </BaseButton>
          </If>
        </View>
      </If>
    </If>
    <Section title="Source" />
    <Stat label="Format" value={props.metadata.format.toUpperCase()} />
    <If when={props.metadata.format === "csv" || props.metadata.format === "tsv"}><Stat label="Header row" value={props.metadata.headers ? "Yes" : "No"} /></If>
    <Stat label="Encoding" value="UTF-8" />
  </View>;
}
