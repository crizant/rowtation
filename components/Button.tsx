import { Text } from "nsh";
import { Button as BaseButton } from "nsh/base";

/** App-owned keyboard, pointer and touch action. */
export function Button(props: {
  label: string;
  onPress: () => void;
  disabled?: boolean;
  quiet?: boolean;
  icon?: boolean;
  testID?: string;
  accessibilityLabel?: string;
}) {
  return <BaseButton onPress={() => props.onPress()} disabled={props.disabled} testID={props.testID} accessibilityLabel={props.accessibilityLabel}
    class={(props.quiet
      ? "cursor-pointer disabled:cursor-not-allowed min-h-11 shrink-0 flex-row items-center justify-center rounded-full border border-border bg-background px-4 py-2 hover:bg-muted active:bg-muted focus:ring-2 focus:ring-ring disabled:opacity-50"
      : "cursor-pointer disabled:cursor-not-allowed min-h-11 shrink-0 flex-row items-center justify-center rounded-full bg-primary px-5 py-2 hover:bg-primary/90 active:bg-primary/80 focus:ring-2 focus:ring-ring disabled:opacity-50") + (props.icon ? " h-11 w-11 px-0" : "")}>
    <Text class={(props.icon ? "text-3xl" : "text-sm font-medium") + (props.quiet ? " text-foreground" : " text-primary-foreground")}>{props.label}</Text>
  </BaseButton>;
}
