# Contributing to AJProxy

Thank you for your interest in contributing to AJProxy! We welcome community contributions to help improve this security tool.

## How to Contribute

To contribute features, bug fixes, or enhancements:

1. **Fork the Repository**: Create a personal copy of the repository on GitHub.
2. **Clone the Fork**: Clone your fork to your local machine.
3. **Create a Branch**: Create a new branch for your feature or bug fix:
   ```bash
   git checkout -b feature/your-feature-name
   ```
4. **Make Changes**: Implement your changes. Ensure code is clean, readable, and conforms to standard Rust styling.
5. **Format and Lint**: Before committing, format the code and run cargo clippy to check for common issues:
   ```bash
   cargo fmt --check
   cargo clippy
   ```
6. **Commit Changes**: Commit your changes with clear, descriptive commit messages:
   ```bash
   git commit -m "feat: add support for custom headers"
   ```
7. **Push to GitHub**: Push your branch to your fork:
   ```bash
   git push origin feature/your-feature-name
   ```
8. **Submit a Pull Request**: Open a Pull Request from your branch in your fork to the main repository's `main` branch.

## Development Setup

AJProxy uses Rust and the `egui` GUI framework.
Ensure you have the required GTK/OpenSSL development libraries installed on Linux before compiling:
```bash
sudo apt update
sudo apt install -y build-essential pkg-config libssl-dev libgtk-3-dev
```

You can build and test your changes locally using:
```bash
cargo build
cargo test
```

## Reporting Issues

If you find a bug or have a suggestion for improvement:
1. Search existing issues to ensure it hasn't been reported.
2. Open a new issue with a clear description, steps to reproduce, and environment details (OS, Rust version).
