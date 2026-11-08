import { Clipboard, effect, signal, Text, View, windowSize } from "nsh";
import { DocumentSession, type ExportOptions } from "rowtation/native";
import { Button } from "./Button";
import { Checkbox } from "./Checkbox";
import { Choice, ChoiceGroup } from "./Choice";
import { Dialog } from "./Dialog";
import { Input } from "./Input";

/** Transient export choices only; the retained Rust session owns every value. */
export function ExportDialog(props: { session: DocumentSession; name: string; onClose: () => void }) {
  const metadata = props.session.metadata();
  const format = signal(metadata.format);
  const filename = signal(props.name === "Pasted data" ? "export" : props.name);
  const headers = signal(true), safe = signal(true), pretty = signal(true), reconstruct = signal(true), sorted = signal(false);
  const delimiter = signal(","), accepted = signal(false), busy = signal(false), message = signal("");
  const delimited = () => format() === "csv" || format() === "tsv";
  const options = (): ExportOptions => ({ format: format(), headers: headers(), safeText: delimited() && safe(),
    delimiter: format() === "tsv" ? "\t" : delimiter(), pretty: pretty(), reconstruct: reconstruct(), sorted: sorted(), allowLossy: accepted() });
  const warning = signal("");
  effect(() => warning.set(props.session.exportWarning({ format: format(), safeText: delimited() && safe(), reconstruct: reconstruct() })));
  function changed() { accepted.set(false); message.set(""); }
  async function perform(copy: boolean) {
    if (busy()) return;
    busy.set(true); message.set("");
    try {
      // Keep serialization and browser invocation in this press, before any await.
      if (copy) {
        const text = props.session.serializeText(options());
        await Clipboard.writeText(text);
        message.set("Copied export text");
      } else {
        const name = props.session.download(filename(), options());
        message.set(`Download requested: ${name}. Check your browser’s downloads.`);
      }
    } catch (error) { message.set(`Export failed: ${String(error)} Your document is unchanged.`); }
    finally { busy.set(false); }
  }
  return <Dialog title={`Export ${metadata.rowCount.toLocaleString()} ${metadata.rowCount === 1 ? "row" : "rows"}`}
    description="Choose a format and download or copy the full document." onClose={() => { if (!busy()) props.onClose(); }}
    footer={<View class="flex-row flex-wrap items-center gap-2">
      <Button label="Copy" quiet disabled={busy() || (!!warning() && !accepted())} onPress={() => void perform(true)} />
      <View class="flex-1" />
      <Button label="Cancel" quiet disabled={busy()} onPress={() => props.onClose()} />
      <Button fullWidth={windowSize().width < 640} label={busy() ? "Exporting…" : "Download"} disabled={busy() || !filename().trim() || (!!warning() && !accepted())} onPress={() => void perform(false)} />
    </View>}>
    <ChoiceGroup>
      <Each of={["csv", "tsv", "json", "ndjson", "xml"]}>{value =>
        <Choice mark={value().toUpperCase()} label={value().toUpperCase()} checked={format() === value()} disabled={busy()} onSelect={() => { format.set(value()); changed(); }} />
      }</Each>
    </ChoiceGroup>
    <View class="gap-2">
      <Text class="text-xs font-medium text-muted-foreground">Filename · extension follows format</Text>
      <Input label="Export filename" testID="export-filename" value={filename()} onChange={value => filename.set(value)} />
    </View>
    <View class="gap-1">
      <Checkbox label="Use current sorted order" checked={sorted()} disabled={busy()} onChange={value => sorted.set(value)} />
      <Text class="text-xs text-muted-foreground">{`All ${metadata.rowCount.toLocaleString()} rows · ${sorted() ? "current sorted order" : "original order"}. Search highlights do not filter export.`}</Text>
    </View>
    <View class="gap-1 border-t border-border pt-2">
      <If when={delimited()}>
        <Checkbox label="Include column headers" checked={headers()} disabled={busy()} onChange={value => headers.set(value)} />
        <If when={format() === "csv"}>
          <Text class="text-xs text-muted-foreground">CSV separator</Text>
          <Input label="CSV separator" value={delimiter()} onChange={value => { delimiter.set(value); changed(); }} />
        </If>
        <Checkbox label="Formula-safe text" checked={safe()} disabled={busy()} onChange={value => { safe.set(value); changed(); }} />
      </If>
      <If when={(format() === "json" || format() === "ndjson") && metadata.format !== "csv" && metadata.format !== "tsv"}>
        <Checkbox label="Reconstruct source structure" checked={reconstruct()} disabled={busy()} onChange={value => { reconstruct.set(value); changed(); }} />
      </If>
      <If when={format() === "json" || format() === "xml"}>
        <Checkbox label="Readable formatting" checked={pretty()} disabled={busy()} onChange={value => pretty.set(value)} />
      </If>
      <If when={format() === "ndjson"}><Text class="text-xs text-muted-foreground">One compact JSON object per line, with a final newline.</Text></If>
    </View>
    <If when={warning()}>
      <View class="gap-2 rounded-xl border border-border bg-muted/40 p-3">
        <Text class="text-[13px] text-foreground">{warning()}</Text>
        <Checkbox label="Accept these mapping changes" checked={accepted()} disabled={busy()} onChange={value => accepted.set(value)} />
      </View>
    </If>
    <Text class="text-xs text-muted-foreground">Export keeps your edits and undo history. It does not reset the imported baseline. Large exports may pause the tab or exceed available memory.</Text>
    <If when={message()}><Text role="alert" testID="export-status" class="text-[13px] text-foreground">{message()}</Text></If>
  </Dialog>;
}
