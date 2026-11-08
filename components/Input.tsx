import { Input as BaseInput } from "nsh/base";

/** Consistent styled input; imported text is always literal text. */
export function Input(props: { value: string; onChange: (value: string) => void; label: string; placeholder?: string; multiline?: boolean; testID?: string; autoFocus?: boolean; submitBehavior?: "submit" | "blurAndSubmit" | "newline"; onSubmit?: () => void; onKeyPress?: (key: string) => void }) {
  return <BaseInput value={props.value} onChange={event => props.onChange(event.value)} accessibilityLabel={props.label}
    submitBehavior={props.submitBehavior} autoFocus={props.autoFocus} onSubmit={() => props.onSubmit?.()} onKeyPress={e => props.onKeyPress?.(e.key)} placeholder={props.placeholder} multiline={props.multiline}
    scrollbar={props.multiline ? { mode: "always", vertical: { class: "absolute inset-y-0 right-0 w-2 p-0.5", thumbClass: "rounded-full bg-foreground/45 dark:bg-foreground/55" } } : false}
    autoCapitalize="none" autoCorrect={false} testID={props.testID}
    class={props.multiline
      ? "h-40 w-full overflow-hidden rounded-xl border border-border bg-muted/30 p-3 font-mono text-sm leading-5 text-foreground focus:border-ring focus:ring-1 focus:ring-ring"
      : "min-h-11 w-full rounded-xl border border-border bg-muted/30 px-3 text-sm text-foreground focus:border-ring focus:ring-1 focus:ring-ring"} />;
}
