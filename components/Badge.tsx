import { Text, View } from "nsh";

export function Badge(props: { label: string }) {
  return <View class="shrink-0 rounded-md bg-muted px-2 py-0.5">
    <Text class="text-xs font-medium text-muted-foreground">{props.label}</Text>
  </View>;
}
