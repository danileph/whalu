# Agent instructions

## Branches

- Use `develop` for ongoing development. Start feature branches from `develop` and merge them back there.
- Keep `main` for published changes. Promote `develop` to `main` when the user requests a release or publication; GitHub Pages deploys only from `main`.
- Stable releases use `vX.Y.Z` tags on commits already in `main`. Keep `Cargo.toml` and `assets/Info.plist` versions aligned before tagging.

## Apply each development iteration

- After every iteration that changes application code, dependencies, bundled assets, or build configuration, run the appropriate checks, rebuild the release app with `./scripts/build-app.sh`, and reinstall it at `~/Applications/Whalu.app` before reporting completion.
- Quit the running installed app before replacing it, preserve the previous bundle for rollback, then launch the newly installed app. Verify its signature and confirm that the installed executable matches the freshly built executable.
- After each reinstall, reset and refresh Whalu's existing Accessibility and Screen & System Audio Recording permissions for the newly signed bundle. Refresh Input Monitoring too when it is used. Scope resets to Whalu (`dev.danileph.whalu`), preserving other applications' grants.
- Complete the permission reset/regrant workflow, including removing stale entries and re-adding the installed bundle or using Quit & Reopen when required. An enabled toggle for an older signature is insufficient.
- Verify that the installed GUI app reaches its running state and that existing window-preview access works. Terminal diagnostics can inherit the terminal's permissions and do not establish that the installed app is authorized.
- Rebuilding, reinstalling, and refreshing existing permissions are part of the authorized development workflow; do not ask for separate confirmation each time. If macOS requires the user's authentication or a permission action that tools cannot complete, request only that necessary action and clearly report that activation remains pending. Do not describe an app waiting for permissions as fully applied or ready to use.
- Documentation-only iterations that do not change the app bundle require the documentation checks below; they do not require rebuilding, reinstalling, or resetting permissions.

## Commits

- Commit each completed, coherent change after running the appropriate checks. Do not leave completed work uncommitted.
- Follow Conventional Commits, consistent with the repository history: `<type>[optional scope]: <summary>` (for example, `fix(scroll): delay mouse-wheel acceleration`).
- Use an appropriate type: `feat`, `fix`, `docs`, `refactor`, `perf`, `test`, `build`, `ci`, or `chore`.
- Write a concise, imperative summary describing the change. Add a body when the reason or tradeoffs need explanation.
- Keep commits focused. Stage only files belonging to the change; preserve unrelated existing work.
- Run checks appropriate to the change before committing. For Rust changes, use `cargo fmt --check`, `cargo test`, and `cargo clippy --all-targets -- -D warnings`. For documentation-only changes, use `git diff --check`.
- Do not amend commits or rewrite history unless the user explicitly requests it.
