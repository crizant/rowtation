import { Input as BaseInput } from "nsh/base";

/** Consistent styled input; imported text is always literal text. */
export function Input(props: { value: string; onChange: (value: string) => void; label: string; placeholder?: string; multiline?: boolean; testID?: string }) {
  return <BaseInput value={props.value} onChange={event => props.onChange(event.value)} accessibilityLabel={props.label}
    placeholder={props.placeholder} multiline={props.multiline} autoCapitalize="none" autoCorrect={false} testID={props.testID}
    class={props.multiline
      ? "h-40 w-full rounded-xl border border-border bg-muted/30 p-3 text-sm text-foreground focus:ring-2 focus:ring-ring"
      : "min-h-11 w-full rounded-xl border border-border bg-muted/30 px-3 text-sm text-foreground focus:ring-2 focus:ring-ring"} />;
}
