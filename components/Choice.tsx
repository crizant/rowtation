import { Text, View, windowSize, type JSX } from "nsh";
import { Radio, RadioGroup } from "nsh/base";

/** A row of mutually exclusive tiles with radio semantics and arrow-key movement. */
export function ChoiceGroup(props: { children?: JSX.Element }) {
  return <RadioGroup class="flex-row flex-wrap gap-2">{props.children}</RadioGroup>;
}

/** A tile with a small format mark above its label; the checked tile gets a strong outline. */
export function Choice(props: { mark: string; label: string; checked: boolean; disabled?: boolean; onSelect: () => void }) {
  return <Radio checked={props.checked} disabled={props.disabled} onChange={() => props.onSelect()} accessibilityLabel={`${props.mark} · ${props.label}`}
    class={(windowSize().width < 640 ? "min-w-24 " : "min-w-16 ") + "group cursor-pointer disabled:cursor-not-allowed flex-1 min-h-11 items-center gap-2 rounded-xl border border-border bg-background px-2 py-3 hover:bg-muted/40 checked:border-2 checked:border-foreground focus:ring-2 focus:ring-ring disabled:opacity-50"}>
    <View class="h-9 w-8 items-center justify-end rounded-md bg-muted pb-1 group-checked:bg-primary">
      <Text class="text-[10px] font-semibold text-muted-foreground group-checked:text-primary-foreground">{(props.mark === "NDJSON" ? "NDJ" : props.mark)}</Text>
    </View>
    <Text class="text-[13px] font-medium text-foreground">{props.label}</Text>
  </Radio>;
}
