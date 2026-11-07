import { mount, Text, View } from "nsh";

function App() {
  return (
    <View class="flex-1 pt-[env(safe-area-inset-top)] pr-[env(safe-area-inset-right)] pb-[env(safe-area-inset-bottom)] pl-[env(safe-area-inset-left)] items-center justify-center gap-4 bg-background">
      <Text class="text-2xl font-semibold text-foreground">Rowtation</Text>
      <Text class="text-lg text-muted-foreground">Import and table view are coming next.</Text>
    </View>
  );
}

mount(App);
