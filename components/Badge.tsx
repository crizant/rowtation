import { Text, View } from "nsh";

export function Badge(props: { label: string }) {
  return <View class="shrink-0 rounded-full border border-border bg-background px-3 py-1">
    <Text class="text-xs font-medium text-muted-foreground">{props.label}</Text>
  </View>;
}
