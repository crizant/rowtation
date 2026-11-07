import { Text, View, type JSX } from "nsh";
import { Button as BaseButton } from "nsh/base";
import { Icon, type IconName } from "./Icon";

/** A grouped, rounded list of navigation-style actions separated by hairlines. */
export function ActionList(props: { children?: JSX.Element }) {
  return <View class="overflow-hidden rounded-2xl border border-border/70 bg-background shadow-sm">{props.children}</View>;
}

/** One row of an ActionList: icon tile, label and a disclosure caret. */
export function ActionRow(props: { label: string; icon: IconName; onPress: () => void; last?: boolean }) {
  return <BaseButton onPress={() => props.onPress()}
    class={"cursor-pointer min-h-14 flex-row items-center gap-3 px-4 py-2 hover:bg-muted/50 active:bg-muted focus:ring-2 focus:ring-ring" + (props.last ? "" : " border-b border-border/70")}>
    <View class="h-8 w-8 items-center justify-center rounded-lg bg-primary">
      <Icon name={props.icon} color="primary-foreground" class="h-4 w-4" />
    </View>
    <Text class="flex-1 text-[15px] font-medium text-foreground">{props.label}</Text>
    <Icon name="caret-right" color="muted-foreground" class="h-4 w-4" />
  </BaseButton>;
}
