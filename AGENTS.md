# AGENTS.md

## Branches

- `main` holds releases; `develop` is the integration branch.
- Each issue gets a branch `<issue number>-<short-slug>` (e.g. `2-first-run-tools`) cut from up-to-date `develop`; PRs target `develop` and land by **Squash and merge**, so the PR title becomes the one commit release-please reads: one changelog line per PR.
- `develop` is merged into `main` only for a release, by **Create a merge commit**; a squash would cut `develop`'s history off from `main`.

## Before committing

Run `npm run check` (ESLint, Prettier, `tsc`, Vitest, clippy, rustfmt, `cargo test`) and commit only when it passes. CI runs the same checks on every PR.

## Commits

Every commit and every PR title, including an agent's, follows [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/): `<type>(<scope>)?: <description>`. release-please derives the next version and `CHANGELOG.md` from these messages (`feat` → minor, `fix`/`perf` → patch, `!` → major, or minor while the version is below 1.0), and commitlint rejects non-conforming ones in the `commit-msg` hook and, with the PR title, in CI.

- **Types**: `feat`, `fix`, `refactor`, `perf`, `style`, `test`, `docs`, `build`, `ci`, `chore`, `revert`.
- **Scopes** (optional): `ui` (React frontend in `src/`), `backend` (Rust in `src-tauri/src/`), `ytdlp`, `transcript`, `settings`, `deps`.
- **Description**: English, imperative mood, lowercase, no trailing period, ≤72 chars. Example: `feat(ui): add format picker tabs`.
- **Body** (optional): explain _why_, wrapped at 72 chars.
- **Breaking changes**: `!` after the type/scope and a `BREAKING CHANGE:` footer.
- One logical change per commit.

## Agent skills

### Issue tracker

Issues are tracked in GitHub Issues (yozhikOm/download-video-app) via the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

Default vocabulary: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: one `GLOSSARY.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.
