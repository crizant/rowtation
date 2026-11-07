import { Scroll, Text, View, windowSize, type JSX } from "nsh";
import { Dialog as BaseDialog } from "nsh/base";

/** Centered dialog on desktop; an accessible bottom panel on narrow browsers. */
export function Dialog(props: { title: string; description?: string; onClose: () => void; children?: JSX.Element }) {
  return <BaseDialog testID="app-dialog" animation="none" onOpenChange={e => { if (!e.open) props.onClose(); }}
    backdropClass="bg-foreground/20"
    style={windowSize().width < 640
      ? { position: "absolute", bottom: 0, left: 0, right: 0, maxHeight: windowSize().height * 0.9 }
      : { position: "absolute", left: "50%", top: "50%", translate: "-50% -50%", maxHeight: windowSize().height * 0.9 }}
    class={windowSize().width < 640
      ? "absolute bottom-0 w-full max-h-[90%] rounded-t-3xl border border-border bg-background p-5"
      : "w-[520px] max-w-[92%] max-h-[90%] rounded-3xl border border-border/70 bg-background p-6 shadow-xl"}>
    <Scroll class="min-h-0" scrollbar={false}><View class="gap-5 min-h-0 pb-[env(safe-area-inset-bottom)]">
      <View class="gap-1">
        <Text class="text-base font-semibold text-foreground">{props.title}</Text>
        <If when={props.description}><Text class="text-[13px] text-muted-foreground">{props.description}</Text></If>
      </View>
      {props.children}
    </View></Scroll>
  </BaseDialog>;
}
