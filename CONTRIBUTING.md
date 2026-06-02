# Contributing to OpenZone Rustaceans

Thanks for your interest in contributing! 🎉

## How to Contribute

1. **Fork** the repository and create your branch from `main`.
2. **Make your changes** with clear, focused commits.
3. **Run checks** before opening a pull request.
4. **Open a Pull Request** describing what you changed and why.

## Local Checks

```bash
cargo fmt --check
cargo clippy --all-targets --all-features
cargo test
```

## Reporting Bugs

Open an [issue](https://github.com/bengidev/openzone-rustaceans/issues) with:

- A clear description of the problem
- Steps to reproduce
- Your OS and app version or commit
- Logs, screenshots, or terminal output if relevant

## Feature Requests

Open an issue and describe:

- The workflow or user problem
- The expected assistant behavior
- Any privacy, security, or desktop-permission concerns
- Suggested implementation direction, if known

## Code Style

- Rust: run `cargo fmt` and `cargo clippy`.
- Keep changes focused and well documented.
- Avoid committing generated build artifacts.
- Do not commit secrets, API keys, tokens, or private local config.

## Architecture Notes

- The app is currently a single Cargo package, not a workspace of publishable crates.
- Keep feature logic inside vertical feature modules such as `src/features/onboarding`.
- Keep reusable internal primitives in `src/shared`.
- Preserve domain boundaries: contracts in `domain`, reducers/use-case state in `application`, adapters in `infrastructure`, Iced rendering in `presenter`.
- Do not create standalone crates unless there is a clear external consumer or publishable API.
- See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Commit Messages

Use clear, conventional messages where possible:

- `feat:` for new features
- `fix:` for bug fixes
- `docs:` for documentation
- `chore:` for maintenance
- `refactor:` for structural changes without behavior changes

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
