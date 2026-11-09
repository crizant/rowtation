import { platform, Scroll, signal, Text, View, windowSize } from "nsh";
import { Button } from "./Button";
import { Dialog } from "./Dialog";
import { DialogActions } from "./DialogActions";
import { bytes } from "./format";

/** UTF-16 units rendered per step, so a value of any length opens without laying out all of it. */
const PAGE = 32_768;

/** A prefix that never splits a surrogate pair. */
function prefix(text: string, end: number) {
  if (end >= text.length) return text;
  const code = text.charCodeAt(end - 1);
  return text.slice(0, code >= 0xd800 && code <= 0xdbff ? end - 1 : end);
}

/** The whole selected value, rendered in steps; copying always takes all of it. */
export function ValueDialog(props: { text: string; byteLength: number; title: string; copyLabel: string; onCopy: () => void; onClose: () => void }) {
  const shown = signal(PAGE);
  const more = () => shown() < props.text.length;
  return <Dialog title={props.title} description={bytes(props.byteLength)} onClose={() => props.onClose()} footer={<DialogActions
    secondary={<Button label={props.copyLabel} testID="copy-full-value" quiet compact={platform.desktop && windowSize().width >= 640} onPress={() => props.onCopy()} />}
    primary={<Button label="Done" fullWidth={windowSize().width < 640} compact={platform.desktop && windowSize().width >= 640} onPress={() => props.onClose()} />} />}>
    <Scroll testID="full-value" style={{ maxHeight: windowSize().height * 0.45 }} class="min-h-0 rounded-xl border border-border/70 bg-muted/30">
      <Text class="p-3 font-mono text-[13px] text-foreground">{prefix(props.text, shown()) || "(empty text)"}</Text>
    </Scroll>
    <If when={more()}>
      <View class="flex-row flex-wrap items-center gap-3">
        <Text class="flex-1 text-xs text-muted-foreground">{`Showing ${prefix(props.text, shown()).length.toLocaleString()} of ${props.text.length.toLocaleString()} characters. Copy value copies all of it.`}</Text>
        <Button label="Show more" quiet compact={platform.desktop && windowSize().width >= 640} onPress={() => shown.set(shown() * 2)} />
      </View>
    </If>
  </Dialog>;
}
