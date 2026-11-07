import { Image, token } from "nsh";
import caretLeft from "./icons/caret-left.svg";

/** Phosphor icons (regular weight, MIT), vendored in `components/icons`. */
const icons = {
  "caret-left": caretLeft,
};

export type IconName = keyof typeof icons;

/** Decorative icon recolored with a theme token; label its control instead. */
export function Icon(props: { name: IconName; color?: "foreground" | "muted-foreground" | "primary-foreground"; class?: string }) {
  return <Image src={icons[props.name]} accessibilityElementsHidden class={"shrink-0 " + (props.class ?? "h-5 w-5")}
    style={{ pointerEvents: "none", tintColor: token(props.color ?? "foreground") }} />;
}
