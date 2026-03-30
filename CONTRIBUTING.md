# Contributing

Thank you for your interest in contributing to Chai (:

This document provides guidelines for contributing to the project.

## Code Formatting

This project uses **default rustfmt settings**. To ensure consistent code style across all contributions:

1. **Check formatting locally** before pushing:

   ```sh
   cargo fmt -- --check
   ```

2. **Fix formatting** if the check fails:

   ```sh
   cargo fmt
   ```

3. **CI will block PRs** with incorrect formatting. Fix any issues before merging.

**Important**: Do not create a `rustfmt.toml` file or override rustfmt settings. Chai uses the default configuration to ensure consistency.

## Development Workflow

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run `cargo fmt -- --check` to verify formatting
5. Run `cargo build` and `cargo test` to verify everything works
6. Submit a pull request and link it to the relevant issue

## Questions?

Feel free to open an issue if you have questions about contributing!
