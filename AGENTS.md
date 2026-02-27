Use this as your checklist template for all future tasks. Place it at the **end** of any task checklist you maintain on the side (e.g. Antigravity's task.md artifact), and use it as your final checklist before declaring your work complete:

- [ ] Use @docs/plan.md for planning
- [ ] Keep @docs/plan.md checkmarks up to date.
- [ ] Use a TDD approach to implement new features, and fix bugs.
- [ ] Ensure the following pass with no warnings or errors:
    - `cargo clippy --all-targets --all-features -- -D warnings`
    - `cargo test`
    - `cargo fmt -- --check`
- [ ] Prefer "Plain English" and jargon-free explanations in documentation, comments, names in code, and commit messages. Exception: technical terms High School students would understand are fine, as are Computer Science terms a university student would understand.
- [ ] Avoid cryptic variable names; prefer words.
