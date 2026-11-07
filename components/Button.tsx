import { Text } from "nsh";
import { Button as BaseButton } from "nsh/base";
import { Icon, type IconName } from "./Icon";

/** App-owned keyboard, pointer and touch action. With `icon`, a round icon button labelled by `label`. */
export function Button(props: {
  label: string;
  onPress: () => void;
  disabled?: boolean;
  quiet?: boolean;
  icon?: IconName;
  testID?: string;
  accessibilityLabel?: string;
}) {
  return <BaseButton onPress={() => props.onPress()} disabled={props.disabled} testID={props.testID} accessibilityLabel={props.accessibilityLabel ?? (props.icon ? props.label : undefined)}
    class={(props.quiet
      ? "cursor-pointer disabled:cursor-not-allowed min-h-11 shrink-0 flex-row items-center justify-center rounded-full border border-border bg-background px-4 py-2 hover:bg-muted active:bg-muted focus:ring-2 focus:ring-ring disabled:opacity-50"
      : "cursor-pointer disabled:cursor-not-allowed min-h-11 shrink-0 flex-row items-center justify-center rounded-full bg-primary px-5 py-2 hover:bg-primary/90 active:bg-primary/80 focus:ring-2 focus:ring-ring disabled:opacity-50") + (props.icon ? " h-11 w-11 px-0 py-0" : "")}>
    {props.icon
      ? <Icon name={props.icon} color={props.quiet ? "foreground" : "primary-foreground"} />
      : <Text class={"text-sm font-medium" + (props.quiet ? " text-foreground" : " text-primary-foreground")}>{props.label}</Text>}
  </BaseButton>;
}
