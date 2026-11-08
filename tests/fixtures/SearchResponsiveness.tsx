// Synthetic large input: all matching and cursor state stay in Rust/WASM.
import { cleanup, mount, signal, Text, View } from "nsh";
import { DocumentSession, ImportSource } from "rowtation/native";
import { Button } from "../../components/Button";

function SearchResponsiveness() {
  const source = ImportSource.fromText("value\n" + ("x".repeat(80) + "\n").repeat(500_000) + "target\n", "search.csv");
  const session = DocumentSession.fromSource(source, {});
  source.dispose();
  const status = signal("Ready");
  const timing = signal("");
  let timer: ReturnType<typeof setTimeout> | undefined;
  let heartbeat: ReturnType<typeof setTimeout> | undefined;
  let generation = 0;
  cleanup(() => { generation++; clearTimeout(timer); clearTimeout(heartbeat); session.dispose(); });
  function start(restart: boolean) {
    const run = ++generation;
    clearTimeout(timer); clearTimeout(heartbeat);
    session.startSearch({ term: "absent" });
    status.set("Searching");
    let slices = 0, ticks = 0, longest = 0, scanning = 0;
    const began = performance.now();
    const step = () => {
      if (run !== generation) return;
      const started = performance.now();
      const progress = session.searchStep();
      const elapsed = performance.now() - started;
      slices++; longest = Math.max(longest, elapsed); scanning += elapsed;
      if (progress.result) {
        if (restart) {
          const match = session.searchMatch(0);
          status.set(progress.result.count === 1 && match?.rowId === 500_001 ? "Restart correct" : "Restart failed");
        } else status.set(ticks > 0 && slices > 1 && progress.result.count === 0 ? "Yielded between slices" : "No yield");
        const report = `${slices} slices; longest ${longest.toFixed(1)} ms; scanning ${scanning.toFixed(1)} ms; elapsed ${(performance.now() - began).toFixed(1)} ms`;
        timing.set(report);
        console.log("Search responsiveness: " + report);
        return;
      }
      // This separate task must run while Rust still has unfinished work. On
      // restart it cancels a real in-flight scan, then changes the query.
      heartbeat = setTimeout(() => {
        ticks++;
        if (restart && ticks === 1) {
          session.cancelSearch();
          session.startSearch({ term: "target" });
        }
      }, 0);
      timer = setTimeout(step, 0);
    };
    timer = setTimeout(step, 0);
  }
  return <View><Text>{status()}</Text><Text>{timing()}</Text>
    <Button label="Search" onPress={() => start(false)} />
    <Button label="Restart" onPress={() => start(true)} />
  </View>;
}
mount(SearchResponsiveness);
