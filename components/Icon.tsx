import { Image, token } from "nsh";
import arrowClockwise from "./icons/arrow-clockwise.svg";
import arrowCounterClockwise from "./icons/arrow-counter-clockwise.svg";
import caretLeft from "./icons/caret-left.svg";
import caretRight from "./icons/caret-right.svg";
import check from "./icons/check.svg";
import clipboardText from "./icons/clipboard-text.svg";
import copy from "./icons/copy.svg";
import dotsThree from "./icons/dots-three.svg";
import folderOpen from "./icons/folder-open.svg";
import info from "./icons/info.svg";
import magnifyingGlass from "./icons/magnifying-glass.svg";
import uploadSimple from "./icons/upload-simple.svg";
import warning from "./icons/warning.svg";
import x from "./icons/x.svg";

/** Phosphor icons (regular weight, MIT), vendored in `components/icons`. */
const icons = {
  "arrow-clockwise": arrowClockwise,
  "arrow-counter-clockwise": arrowCounterClockwise,
  "caret-left": caretLeft,
  "caret-right": caretRight,
  check,
  "clipboard-text": clipboardText,
  copy,
  "dots-three": dotsThree,
  "folder-open": folderOpen,
  info,
  "magnifying-glass": magnifyingGlass,
  "upload-simple": uploadSimple,
  warning,
  x,
};

export type IconName = keyof typeof icons;

/** Decorative icon recolored with a theme token; label its control instead. */
export function Icon(props: { name: IconName; color?: "foreground" | "muted-foreground" | "primary-foreground"; class?: string }) {
  return <Image src={icons[props.name]} accessibilityElementsHidden class={"shrink-0 " + (props.class ?? "h-5 w-5")}
    style={{ pointerEvents: "none", tintColor: token(props.color ?? "foreground") }} />;
}
