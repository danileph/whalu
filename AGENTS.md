# Agent instructions

## Commits

- Commit each completed, coherent change after running the appropriate checks. Do not leave completed work uncommitted.
- Follow Conventional Commits, consistent with the repository history: `<type>[optional scope]: <summary>` (for example, `fix(scroll): delay mouse-wheel acceleration`).
- Use an appropriate type: `feat`, `fix`, `docs`, `refactor`, `perf`, `test`, `build`, `ci`, or `chore`.
- Write a concise, imperative summary describing the change. Add a body when the reason or tradeoffs need explanation.
- Keep commits focused. Stage only files belonging to the change; preserve unrelated existing work.
- Run checks appropriate to the change before committing. For Rust changes, use `cargo fmt --check`, `cargo test`, and `cargo clippy --all-targets -- -D warnings`. For documentation-only changes, use `git diff --check`.
- Do not amend commits or rewrite history unless the user explicitly requests it.
