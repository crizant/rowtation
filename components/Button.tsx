import { Text } from "nsh";
import { Button as BaseButton } from "nsh/base";

/** The app's primary action, with keyboard, pointer and touch support. */
export function Button(props: {
  label: string;
  onPress: () => void;
  disabled?: boolean;
}) {
  return (
    <BaseButton
      onPress={props.onPress}
      disabled={props.disabled}
      class="min-h-11 items-center justify-center rounded-(--radius) bg-primary px-5 py-3 hover:bg-primary/90 active:bg-primary/80 focus:ring-2 focus:ring-ring disabled:opacity-50"
    >
      <Text class="text-lg text-primary-foreground">{props.label}</Text>
    </BaseButton>
  );
}
