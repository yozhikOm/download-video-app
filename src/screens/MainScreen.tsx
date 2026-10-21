import { useEffect, useState } from "react";
import {
  Card,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { appInfo as fetchAppInfo, type AppInfo } from "@/backend";
import { strings } from "@/strings";

export function MainScreen() {
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    fetchAppInfo()
      .then(setAppInfo)
      .catch(() => setFailed(true));
  }, []);

  return (
    <>
      {failed && (
        <p role="alert" className="text-destructive">
          {strings.main.versionFailed}
        </p>
      )}
      {appInfo && (
        <Card className="w-full max-w-sm">
          <CardHeader>
            <CardTitle>
              <h1>{appInfo.name}</h1>
            </CardTitle>
            <CardDescription>
              {strings.main.version(appInfo.version)}
            </CardDescription>
          </CardHeader>
        </Card>
      )}
    </>
  );
}
