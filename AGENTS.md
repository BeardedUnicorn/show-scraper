# Repository Guidelines

## Project Structure & Module Organization
- `src/` contains the React entrypoints (`main.tsx`, `App.tsx`) and shared assets in `src/assets/`.
- `public/` hosts static resources served by Vite; `index.html` is the HTML shell.
- `src-tauri/` houses the Rust-side Tauri app (`src/`, `Cargo.toml`, `tauri.conf.json`) plus app icons.
- Use `SCRAPE.md` and `OVERVIEW.md` for scraping context before architecting new features.

## Build, Test, and Development Commands
- `npm install` pulls JavaScript dependencies and the Tauri CLI binaries.
- `npm run dev` starts the Vite dev server with hot reload on `http://localhost:5173`.
- `npm run build` type-checks with `tsc` then emits a production bundle into `dist/`.
- `npm run preview` serves the built bundle for pre-release verification.
- `npm run tauri dev` runs the desktop shell against the current frontend build; use after verifying `npm run build`.

## Coding Style & Naming Conventions
- Write TypeScript with functional React components; prefer hooks over class components.
- Use PascalCase for component files, camelCase for utilities, and kebab-case for assets.
- Stick to two-space indentation and single quotes unless JSX syntax requires double quotes.
- Run `npm run build` before committing to surface type errors and lint-like feedback; run `cargo fmt` inside `src-tauri/` for Rust formatting.

## Testing Guidelines
- No automated suite yet; when adding UI logic, place React Testing Library specs in `src/__tests__/` using `<Component>.test.tsx`.
- Stub network or Tauri API calls via `@tauri-apps/api` mocks to keep tests deterministic.
- For Rust commands, add unit tests in `src-tauri/src/` and execute `cargo test`; ensure new logic has at least smoke coverage.

## Commit & Pull Request Guidelines
- Use concise, imperative commit subjects (e.g. `Add venue list parser`) and keep unrelated changes separate.
- Reference issues with `Fixes #ID` in commit or PR descriptions; note both frontend and Rust impacts when relevant.
- Pull requests should outline scope, list manual verification steps (`npm run dev`, `npm run tauri dev`), and include screenshots for UI-facing updates.

## Tauri & Environment Notes
- Update `tauri.conf.json` when relocating assets so window bundles resolve correct paths.
- Align dependency versions between `package.json` and `src-tauri/Cargo.toml` when introducing new plugins or APIs to avoid runtime drift.
