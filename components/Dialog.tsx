import { Scroll, Text, View, windowSize, type JSX } from "nsh";
import { Dialog as BaseDialog } from "nsh/base";

/** Centered dialog on desktop; an accessible bottom panel on narrow browsers. */
export function Dialog(props: { title: string; description?: string; onClose: () => void; footer?: JSX.Element; children?: JSX.Element }) {
  return <BaseDialog testID="app-dialog" animation="none" onOpenChange={e => { if (!e.open) props.onClose(); }}
    backdropClass="bg-foreground/20"
    class={windowSize().width < 640
      ? "absolute inset-0 justify-end pointer-events-none"
      : "absolute inset-0 items-center justify-center pointer-events-none"}>
    <View testID="dialog-panel" style={{ maxHeight: windowSize().height * 0.9 }}
      class={windowSize().width < 640
        ? "pointer-events-auto w-full gap-5 rounded-t-3xl border border-border bg-background p-5"
        : "pointer-events-auto w-[520px] max-w-[92%] gap-5 rounded-3xl border border-border/70 bg-background p-6 shadow-xl"}>
      <View class="gap-1 shrink-0">
        <Text class="text-base font-semibold text-foreground">{props.title}</Text>
        <If when={props.description}><Text class="text-[13px] text-muted-foreground">{props.description}</Text></If>
      </View>
      {/* Insets keep field focus rings clear of the scroll clipping edge. */}
      <Scroll testID="dialog-content" class="-mx-1 -my-1 min-h-0 shrink" scrollbar={false}>
        <View class={props.footer ? "gap-5 shrink-0 p-1" : "gap-5 shrink-0 p-1 pb-[env(safe-area-inset-bottom)]"}>{props.children}</View>
      </Scroll>
      <If when={props.footer}><View class="shrink-0 pt-1 -mt-1 pb-[calc(env(safe-area-inset-bottom)+4px)] -mb-1">{props.footer}</View></If>
    </View>
  </BaseDialog>;
}
