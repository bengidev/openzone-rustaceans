# OpenZone Rustaceans

> A Rust-powered desktop AI assistant foundation that connects your desktop workflow with AI models so you can finish work faster without leaving your machine.

![platform](https://img.shields.io/badge/platform-desktop-555555?style=flat&labelColor=2b2b2b)
![Rust](https://img.shields.io/badge/Rust-2024-CE422B?style=flat&labelColor=2b2b2b&logo=rust)
![status](https://img.shields.io/badge/status-early%20development-EBCB8B?style=flat&labelColor=2b2b2b)
![License](https://img.shields.io/badge/License-MIT-EBCB8B?style=flat&labelColor=2b2b2b)

OpenZone Rustaceans is an early-stage desktop AI assistant project. It is being built as a native Rust foundation for integrating desktop context, local workflows, and AI model capabilities into one assistant that can help users plan, write, automate, and complete tasks directly from the desktop.

## ✨ Features

- 🖥️ **Desktop-first assistance** — designed around real desktop workflows instead of isolated browser chat.
- 🤖 **AI model integration** — planned support for connecting model providers and local AI workflows.
- ⚙️ **Rust-native core** — a fast, reliable foundation for system integrations and automation.
- 🔒 **User-controlled context** — privacy-aware architecture where users decide what context is shared.
- 🧩 **Extensible direction** — structured to grow into providers, tools, skills, and desktop actions.

## 🚀 Tech Stack

- **Rust 2024** — application core and desktop/system integration layer
- **Iced** — current native UI prototype runtime
- **Cargo** — build, test, and dependency management
- **Future AI providers** — planned connector layer for cloud and/or local models

## 📦 Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) toolchain
- Cargo, included with Rust

## 🛠️ Getting Started

```bash
# Clone the repository
git clone https://github.com/bengidev/openzone-rustaceans.git
cd openzone-rustaceans

# Run the app prototype
cargo run

# Run checks
cargo fmt --check
cargo clippy --all-targets --all-features
cargo test

# Build a release binary
cargo build --release
```

## 📁 Project Structure

```text
openzone-rustaceans/
├── src/
│   ├── main.rs          # Composition root
│   ├── features/        # Vertical feature modules
│   │   └── onboarding/  # Internal first-run onboarding feature
│   └── shared/          # Shared internal modules, including design tokens
├── Cargo.toml           # Single package metadata and dependencies
├── Cargo.lock           # Locked dependency graph
├── ABOUT.md             # Project overview and vision
├── CONTRIBUTING.md      # Contribution guidelines
├── SECURITY.md          # Security and AI data-handling policy
└── docs/                # Maintainer, architecture, and agent documentation
```

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for current internal module boundaries.

## 🤝 Contributing

Contributions are welcome! Please open an issue to discuss what you'd like to change. See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## 📄 License

Licensed under the [MIT License](LICENSE).
