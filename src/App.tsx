import { useEffect, useState } from "react";
import {
  subscribe,
  type ProbeState,
  type SettingsState,
  type SetupState,
} from "@/backend";
import { MainScreen } from "@/screens/MainScreen";
import { SetupScreen } from "@/screens/SetupScreen";

function App() {
  const [setup, setSetup] = useState<SetupState>({ status: "checking" });
  const [probe, setProbe] = useState<ProbeState>({ status: "idle" });
  const [settings, setSettings] = useState<SettingsState>({
    signInBrowser: null,
  });

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
        }
      }),
    [],
  );

  return (
    <main className="flex min-h-svh items-center justify-center p-6">
      {(setup.status === "installing" || setup.status === "failed") && (
        <SetupScreen state={setup} />
      )}
      {setup.status === "ready" && (
        <MainScreen probe={probe} settings={settings} />
      )}
    </main>
  );
}

export default App;
