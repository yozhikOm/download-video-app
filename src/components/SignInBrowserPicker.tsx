import { useState } from "react";
import { Card } from "@/components/ui/card";
import {
  NativeSelect,
  NativeSelectOption,
} from "@/components/ui/native-select";
import { setSignInBrowser, type Browser } from "@/backend";
import { strings } from "@/strings";

const browsers = Object.keys(strings.browsers) as Browser[];

/**
 * Which browser's YouTube sign-in the app uses, if any: a row of Windows
 * Settings, with what it does on the left and the list on the right.
 */
export function SignInBrowserPicker({
  signInBrowser,
}: {
  signInBrowser: Browser | null;
}) {
  const [saveFailed, setSaveFailed] = useState(false);

  function choose(value: string) {
    setSaveFailed(false);
    setSignInBrowser(value ? (value as Browser) : null).catch(() =>
      setSaveFailed(true),
    );
  }

  return (
    <Card className="flex-row items-center justify-between gap-4 px-4 py-3">
      <div className="flex min-w-0 flex-col gap-0.5">
        <label htmlFor="sign-in-browser" className="font-semibold">
          {strings.signIn.browser}
        </label>
        <p className="text-xs text-muted-foreground">
          {signInBrowser
            ? strings.signIn.warning(strings.browsers[signInBrowser])
            : strings.signIn.hint}
        </p>
        {saveFailed && (
          <p role="alert" className="text-xs text-destructive">
            {strings.signIn.saveFailed}
          </p>
        )}
      </div>
      <NativeSelect
        id="sign-in-browser"
        className="shrink-0"
        value={signInBrowser ?? ""}
        onChange={(event) => choose(event.target.value)}
      >
        <NativeSelectOption value="">{strings.signIn.none}</NativeSelectOption>
        {browsers.map((browser) => (
          <NativeSelectOption key={browser} value={browser}>
            {strings.browsers[browser]}
          </NativeSelectOption>
        ))}
      </NativeSelect>
    </Card>
  );
}
