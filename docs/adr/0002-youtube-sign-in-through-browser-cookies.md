# YouTube sign-in through the browser's cookies, which the app never stores

From some networks YouTube answers every request with a bot check ("Sign in to confirm you're not a bot") that no yt-dlp client gets past without an account, and the user can't always change the network. So the user may choose a Sign-in browser, and every yt-dlp run gets `--cookies-from-browser <browser>`: yt-dlp reads the cookies of that browser's YouTube session on each run. The app never copies, exports or keeps the cookies; it stores only which browser was chosen (`settings.json` in the app data folder). By default no browser is chosen and yt-dlp runs without an account.

## Considered Options

- **Login and password in the app**: YouTube no longer accepts them from yt-dlp, and the app would have to keep the user's password.
- **PO tokens** (proof-of-origin tokens yt-dlp can send without an account): they need a separate token provider plugin, and yt-dlp doesn't promise they get past the bot check; on the network that prompted this decision every client got it.
- **A cookies file the app exports once and keeps**: works when the browser can't be read, but the app would hold a copy of the session that goes stale and is a secret to protect.

## Consequences

- yt-dlp acts as the user's account: with many downloads YouTube may restrict it, so the app warns when a browser is chosen. Videos that need signing in (age-restricted, members-only) become available when the account has access.
- On Windows, Chromium-based browsers (Chrome, Edge, Brave and others) lock their cookies while they run, and recent versions encrypt them so that only the browser can read them, so yt-dlp often can't; the app tells these reasons apart from yt-dlp's error and suggests closing the browser or choosing another one, Firefox being the dependable choice.
