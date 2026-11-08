import { Text, type JSX } from "nsh";
import { Button as BaseButton } from "nsh/base";
import { Icon, type IconName } from "./Icon";

/**
 * App-owned keyboard, pointer and touch action. With `icon`, a round icon button labelled by `label`.
 * `compact` is for pointer-driven desktop toolbars and dialogs; touch layouts keep the 44-point default.
 */
export function Button(props: {
  label: string;
  onPress: () => void;
  disabled?: boolean;
  quiet?: boolean;
  compact?: boolean;
  icon?: IconName;
  tight?: boolean;
  selected?: boolean;
  testID?: string;
  accessibilityLabel?: string;
  tooltip?: string;
}) {
  const size = () => props.icon
    ? (props.compact ? " h-9 w-9" : " h-11 w-11")
    : props.tight
      ? (props.compact ? " h-9 min-w-9 px-2" : " h-11 min-w-11 px-2.5")
      : (props.compact ? " min-h-9 px-4 py-1.5" : " min-h-11 px-5 py-2");
  const tone = () => props.quiet
    ? "cursor-pointer disabled:cursor-not-allowed shrink-0 flex-row items-center justify-center rounded-full border shadow-sm hover:bg-muted active:bg-muted focus:ring-2 focus:ring-ring disabled:opacity-50 " + (props.selected ? "border-foreground bg-muted" : "border-border bg-background")
    : "cursor-pointer disabled:cursor-not-allowed shrink-0 flex-row items-center justify-center rounded-full bg-primary shadow-sm hover:bg-primary/90 active:bg-primary/80 focus:ring-2 focus:ring-ring disabled:opacity-50";
  return <BaseButton onPress={() => props.onPress()} disabled={props.disabled} testID={props.testID} accessibilityLabel={props.accessibilityLabel ?? (props.icon ? props.label : undefined)}
    tooltip={props.tooltip ? <Text class="rounded-md bg-foreground px-2 py-1 text-xs text-background">{props.tooltip}</Text> as JSX.Element : undefined}
    class={tone() + size()}>
    {props.icon
      ? <Icon name={props.icon} color={props.quiet ? "foreground" : "primary-foreground"} class={props.compact ? "h-4 w-4" : "h-5 w-5"} />
      : <Text class={(props.compact ? "text-[13px]" : "text-sm") + " font-medium" + (props.quiet ? " text-foreground" : " text-primary-foreground")}>{props.label}</Text>}
  </BaseButton>;
}
