# Backend architecture

The Rust backend in `src-tauri/src/` owns all state. The React UI only shows the events the backend sends and sends commands back. Terms such as Video, Probe, Format, Tool, Package, Setup and Sign-in browser are defined in [`GLOSSARY.md`](../GLOSSARY.md).

```
lib.rs (Tauri commands, wiring)
  └─ Prober ──┬─ Toolbox ──┬─ Releases   ─ GithubReleases │ FakeReleases
              │            ├─ Processes  ─ SystemProcesses │ FakeProcesses
              │            ├─ Clock      ─ SystemClock │ FixedClock
              │            └─ EventSink
              ├─ Settings ── EventSink
              └─ EventSink  ─ Subscribers (lib.rs) │ RecordingSink
```

## Modules

**`toolbox.rs`: `Toolbox`**
The app's own installed Tools in the app data folder (`tools/`), and the only code that knows which Tools exist and which Package provides each one. It runs Setup: it reads the manifest, checks which Tools are in place, downloads the missing Packages through `Releases`, verifies their SHA-256, and installs them. It also runs a Tool through `Processes`, pointing yt-dlp at the app's own ffmpeg and deno, and at the cookies of the Sign-in browser the caller passes (ADR-0002). When a Tool fails to start, `Toolbox` forgets it and shows the setup screen again. `SetupState` is what the setup screen shows, and every change to it is sent as an event.

**`probe.rs`: `Prober`**
Probes a URL by running `yt-dlp -J` through `Toolbox`, with the Sign-in browser from `Settings` when one is chosen, and keeps what it learned as a `Video` (Formats, Transcript languages, original language, chapters, whether it was a stream) for the Download. Only the latest Probe counts: a late answer to an older one is dropped. `ProbeState` is what the main screen shows: the Video's card, or a `ProbeError` for a playlist or channel, a stream that is live now, a Video that needs signing in, YouTube's bot check, a Sign-in browser whose cookies yt-dlp can't read (and why), a URL whose Video id was cut off, or an unsupported URL, along with the Sign-in browser the Probe ran with. When yt-dlp fails to start, `Prober` drops the Probe and runs Setup to restore it. Its tests (`probe/tests.rs`) feed real yt-dlp answers from `probe/fixtures/` through `FakeProcesses`.

**`settings.rs`: `Settings`**
The app's settings in `settings.json` in the app data folder; today only the Sign-in browser. A missing or unreadable file means the defaults. Every change is saved at once and sent as an event; a change that can't be saved changes nothing. Its tests (`settings/tests.rs`) relaunch on the same folder.

**`releases.rs`: `Releases`**
The only code that talks to the network: it asks for a Package's latest release (`latest_release`) and streams an asset to a writer (`fetch`). It does not know where files go or what a Tool is. `GithubReleases` (`releases/github.rs`) uses the official GitHub releases of yt-dlp, FFmpeg-Builds and deno, and reads each one's checksums file. `FakeReleases` (`releases/fake.rs`) publishes small fake releases, and a test can make it go offline, publish a wrong checksum or point to an unofficial asset.

**`processes.rs`: `Processes`**
The only code that starts programs: `run` takes a path and arguments and waits for the program to exit. It does not know which Tool it is running. `SystemProcesses` starts the program without a console window. `FakeProcesses` (`processes/fake.rs`) records what was started and answers with scripted output, and a test can make a Tool fail to start.

**`clock.rs`: `Clock`**
The current time, so tests can fix it (`FixedClock`).

**`events.rs`: `AppEvent`, `EventSink`**
Events from the backend to the UI. `Subscribers` in `lib.rs` forwards them to the UI channels. In tests, `RecordingSink` collects them.

**`lib.rs`**
Tauri commands (`app_info`, `subscribe`, `retry_setup`, `probe`, `set_sign_in_browser`) and wiring: it creates the `Settings`, the `Toolbox` and the `Prober` with the real implementations and starts Setup in the background on launch.

## Seams

`Releases`, `Processes`, `Clock` and `EventSink` are traits so that `Toolbox` and `Prober` can be tested without the network, real programs or the real time (`toolbox/tests.rs`, `probe/tests.rs`). Code that needs the network or a process goes through one of these traits, never around it.
