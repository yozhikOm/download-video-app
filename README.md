# download-video-app

Windows desktop app (Tauri 2 + React) for downloading videos, audio and transcripts with yt-dlp.

## Development

Requirements: Node.js 24, Rust (stable, with `clippy` and `rustfmt`), and the [Tauri prerequisites](https://tauri.app/start/prerequisites/).

```sh
npm install        # also installs the git hooks
npm run tauri dev  # run the app
npm run check      # every lint, format and test check that CI runs
```

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
