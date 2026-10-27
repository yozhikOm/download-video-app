import { useEffect, useState } from "react";
import {
  subscribe,
  type Download,
  type ProbeState,
  type SettingsState,
  type SetupState,
} from "@/backend";
import { CloseDialog } from "@/components/CloseDialog";
import { MainScreen } from "@/screens/MainScreen";
import { SetupScreen } from "@/screens/SetupScreen";

function App() {
  const [setup, setSetup] = useState<SetupState>({ status: "checking" });
  const [probe, setProbe] = useState<ProbeState>({ status: "idle" });
  const [settings, setSettings] = useState<SettingsState>({
    signInBrowser: null,
  });
  const [queue, setQueue] = useState<Download[]>([]);
  const [closeRequested, setCloseRequested] = useState(false);

  useEffect(
    () =>
      subscribe((event) => {
        switch (event.type) {
          case "setup":
            setSetup(event.data);
            break;
          case "probe":
            setProbe(event.data);
            break;
          case "settings":
            setSettings(event.data);
            break;
          case "queue":
            setQueue(event.data);
            break;
          case "closeRequested":
            setCloseRequested(true);
            break;
        }
      }),
    [],
  );

  return (
    <main className="flex min-h-svh items-center justify-center px-7 py-4.5">
      {(setup.status === "installing" || setup.status === "failed") && (
        <SetupScreen state={setup} />
      )}
      {setup.status === "ready" && (
        <MainScreen probe={probe} settings={settings} queue={queue} />
      )}
      <CloseDialog
        open={closeRequested}
        onDismiss={() => setCloseRequested(false)}
      />
    </main>
  );
}

export default App;
