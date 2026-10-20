// Conventional Commits rules from AGENTS.md.
export default {
  extends: ["@commitlint/config-conventional"],
  rules: {
    "type-enum": [
      2,
      "always",
      [
        "feat",
        "fix",
        "refactor",
        "perf",
        "style",
        "test",
        "docs",
        "build",
        "ci",
        "chore",
        "revert",
      ],
    ],
    "scope-enum": [
      2,
      "always",
      ["ui", "backend", "ytdlp", "transcript", "settings", "deps"],
    ],
    "subject-max-length": [2, "always", 72],
    "body-max-line-length": [1, "always", 72],
  },
};
