# AGENTS.md

## Commits

Use [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/): `<type>(<scope>)?: <description>`.

- **Types**: `feat`, `fix`, `refactor`, `perf`, `style`, `test`, `docs`, `build`, `ci`, `chore`, `revert`.
- **Scopes** (optional): `ui` (React frontend in `src/`), `backend` (Rust in `src-tauri/src/`), `ytdlp`, `transcript`, `settings`, `deps`.
- **Description**: English, imperative mood, lowercase, no trailing period, ≤72 chars. Example: `feat(ui): add format picker tabs`.
- **Body** (optional): explain *why*, wrapped at 72 chars.
- **Breaking changes**: `!` after the type/scope and a `BREAKING CHANGE:` footer.
- One logical change per commit.

## Agent skills

### Issue tracker

Issues are tracked in GitHub Issues (yozhikOm/download-video-app) via the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

Default vocabulary: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: one `GLOSSARY.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.
