import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Card,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";

type AppInfo = {
  name: string;
  version: string;
};

function App() {
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);

  useEffect(() => {
    invoke<AppInfo>("app_info").then(setAppInfo);
  }, []);

  return (
    <main className="flex min-h-svh items-center justify-center p-6">
      {appInfo && (
        <Card className="w-full max-w-sm">
          <CardHeader>
            <CardTitle>
              <h1>{appInfo.name}</h1>
            </CardTitle>
            <CardDescription>Версия {appInfo.version}</CardDescription>
          </CardHeader>
        </Card>
      )}
    </main>
  );
}

export default App;
