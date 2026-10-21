import { useEffect, useState } from "react";
import { subscribe, type SetupState } from "@/backend";
import { MainScreen } from "@/screens/MainScreen";
import { SetupScreen } from "@/screens/SetupScreen";

function App() {
  const [setup, setSetup] = useState<SetupState>({ status: "checking" });

  useEffect(
    () =>
      subscribe((event) => {
        if (event.type === "setup") {
          setSetup(event.data);
        }
      }),
    [],
  );

  return (
    <main className="flex min-h-svh items-center justify-center p-6">
      {(setup.status === "installing" || setup.status === "failed") && (
        <SetupScreen state={setup} />
      )}
      {setup.status === "ready" && <MainScreen />}
    </main>
  );
}

export default App;
