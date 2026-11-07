import { Text } from "nsh";
import { Checkbox as BaseCheckbox } from "nsh/base";

/** A labelled option that applies when its form is submitted. */
export function Checkbox(props: { label: string; checked: boolean; onChange: (checked: boolean) => void; disabled?: boolean }) {
  return <BaseCheckbox checked={props.checked} disabled={props.disabled} onChange={event => props.onChange(event.checked)}
    class="cursor-pointer disabled:cursor-not-allowed min-h-11 flex-row items-center gap-3 rounded-md focus:ring-2 focus:ring-ring disabled:opacity-50"
    indicatorClass="h-5 w-5 items-center justify-center rounded border border-border bg-background checked:border-primary checked:bg-primary"
    indicator={<Text class="text-sm text-primary-foreground">{props.checked ? "✓" : ""}</Text>}>
    <Text class="flex-1 text-sm text-foreground">{props.label}</Text>
  </BaseCheckbox>;
}
