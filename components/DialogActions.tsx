import { View, windowSize, type JSX } from "nsh";

/** Stable action lanes: secondary/cancel above a full-width primary on phones. */
export function DialogActions(props: { secondary?: JSX.Element; cancel?: JSX.Element; primary: JSX.Element }) {
  return <View class={windowSize().width < 640 ? "gap-2" : "flex-row items-center gap-2"}>
    <View class="min-w-0 flex-1 flex-row items-center gap-2">
      {props.secondary}
      <View class="flex-1" />
      {props.cancel}
    </View>
    {props.primary}
  </View>;
}
