# download-video-app

Windows desktop app (Tauri 2 + React) for downloading videos, audio and transcripts with yt-dlp.

## Development

Requirements: Node.js 24, Rust (stable, with `clippy` and `rustfmt`), and the [Tauri prerequisites](https://tauri.app/start/prerequisites/).

```sh
npm install        # also installs the git hooks
npm run tauri dev  # run the app
npm run check      # every lint, format and test check that CI runs
```

## Releases

The app ships only as a Windows NSIS installer, unsigned, so SmartScreen may warn when it starts. Merging release-please's release PR into `main` creates the version tag and its GitHub Release; the `installer` workflow then builds the installer and attaches it to that release. To rebuild the installer of an existing release, run the `installer` workflow by hand with its tag.

```sh
npm run tauri build  # build the installer locally, into src-tauri/target/release/bundle/nsis/
```

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
