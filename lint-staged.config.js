export default {
  "*.{ts,tsx,js}": ["eslint --fix", "prettier --write"],
  "*.{json,css,md,html,yml,yaml}": "prettier --write",
  "src-tauri/**/*.rs": "rustfmt --edition 2024",
};
