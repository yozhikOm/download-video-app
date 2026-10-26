import { useState } from "react";
import { setSignInBrowser, type Browser } from "@/backend";
import { strings } from "@/strings";

const browsers = Object.keys(strings.browsers) as Browser[];

/** Which browser's YouTube sign-in the app uses, if any. */
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
    <section className="flex flex-col gap-1.5 text-sm">
      <div className="flex items-center justify-between gap-4">
        <label htmlFor="sign-in-browser">{strings.signIn.browser}</label>
        <select
          id="sign-in-browser"
          value={signInBrowser ?? ""}
          onChange={(event) => choose(event.target.value)}
          className="h-8 rounded-lg border border-input bg-transparent px-2 outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 dark:bg-input/30"
        >
          <option value="">{strings.signIn.none}</option>
          {browsers.map((browser) => (
            <option key={browser} value={browser}>
              {strings.browsers[browser]}
            </option>
          ))}
        </select>
      </div>
      <p className="text-muted-foreground">
        {signInBrowser
          ? strings.signIn.warning(strings.browsers[signInBrowser])
          : strings.signIn.hint}
      </p>
      {saveFailed && (
        <p role="alert" className="text-destructive">
          {strings.signIn.saveFailed}
        </p>
      )}
    </section>
  );
}
