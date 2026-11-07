import { cleanup, mount, signal, Text, View } from "nsh";
import { Counter } from "rowtation/native";
import { Button } from "./components/Button";

function App() {
  const counter = new Counter();
  cleanup(() => counter.dispose());
  const count = signal(counter.count);
  return (
    <View class="flex-1 pt-[env(safe-area-inset-top)] pr-[env(safe-area-inset-right)] pb-[env(safe-area-inset-bottom)] pl-[env(safe-area-inset-left)] items-center justify-center gap-4 bg-background">
      <Text class="text-2xl font-semibold text-foreground">Rowtation</Text>
      <Text class="text-3xl font-semibold text-foreground">{count()}</Text>
      <Button
        label="Tap"
        onPress={() => count.set(counter.increment())}
      />
    </View>
  );
}

mount(App);
