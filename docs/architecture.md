# Backend architecture

The Rust backend in `src-tauri/src/` owns all state. The React UI only shows the events the backend sends and sends commands back. Terms such as Tool, Package and Setup are defined in [`GLOSSARY.md`](../GLOSSARY.md).

```
lib.rs (Tauri commands, wiring)
  └─ Toolbox ──┬─ Releases   ─ GithubReleases │ FakeReleases
               ├─ Processes  ─ SystemProcesses │ FakeProcesses
               ├─ Clock      ─ SystemClock │ FixedClock
               └─ EventSink  ─ Subscribers (lib.rs) │ RecordingSink
```

## Modules

**`toolbox.rs`: `Toolbox`**
The app's own installed Tools in the app data folder (`tools/`), and the only code that knows which Tools exist and which Package provides each one. It runs Setup: it reads the manifest, checks which Tools are in place, downloads the missing Packages through `Releases`, verifies their SHA-256, and installs them. It also runs a Tool through `Processes`. When a Tool fails to start, `Toolbox` forgets it and shows the setup screen again. `SetupState` is what the setup screen shows, and every change to it is sent as an event.

**`releases.rs`: `Releases`**
The only code that talks to the network: it asks for a Package's latest release (`latest_release`) and streams an asset to a writer (`fetch`). It does not know where files go or what a Tool is. `GithubReleases` (`releases/github.rs`) uses the official GitHub releases of yt-dlp and FFmpeg-Builds. `FakeReleases` (`releases/fake.rs`) publishes small fake releases, and a test can make it go offline, publish a wrong checksum or point to an unofficial asset.

**`processes.rs`: `Processes`**
The only code that starts programs: `run` takes a path and arguments and waits for the program to exit. It does not know which Tool it is running. `SystemProcesses` starts the program without a console window. `FakeProcesses` (`processes/fake.rs`) records what was started, and a test can make a Tool fail to start.

**`clock.rs`: `Clock`**
The current time, so tests can fix it (`FixedClock`).

**`events.rs`: `AppEvent`, `EventSink`**
Events from the backend to the UI. `Subscribers` in `lib.rs` forwards them to the UI channels. In tests, `RecordingSink` collects them.

**`lib.rs`**
Tauri commands (`app_info`, `subscribe`, `retry_setup`) and wiring: it creates the `Toolbox` with the real implementations and starts Setup in the background on launch.

## Seams

`Releases`, `Processes`, `Clock` and `EventSink` are traits so that `Toolbox` can be tested without the network, real programs or the real time (`toolbox/tests.rs`). Code that needs the network or a process goes through one of these traits, never around it.
