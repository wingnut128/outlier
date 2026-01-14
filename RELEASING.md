# Release Process

This project uses [cargo-release](https://github.com/crate-ci/cargo-release) for automated version management and releases.

## Prerequisites

Install cargo-release (one-time setup):
```bash
cargo install cargo-release
```

## Semantic Versioning

This project follows [Semantic Versioning](https://semver.org/):
- **PATCH** (0.1.0 → 0.1.1): Bug fixes, minor changes
- **MINOR** (0.1.0 → 0.2.0): New features, backward compatible
- **MAJOR** (0.1.0 → 1.0.0): Breaking changes

## Release Steps

### 1. Preview the Release (Dry Run)

Always preview before executing:

```bash
# Preview patch release (0.1.0 → 0.1.1)
cargo release patch

# Preview minor release (0.1.0 → 0.2.0)
cargo release minor

# Preview major release (0.1.0 → 1.0.0)
cargo release major
```

This shows you:
- The new version number
- Changes to CHANGELOG.md
- Git operations that will be performed

### 2. Execute the Release

Once you're satisfied with the preview:

```bash
# Execute patch release
cargo release patch --execute

# Execute minor release
cargo release minor --execute

# Execute major release
cargo release major --execute
```

### 3. What Happens Automatically

When you execute a release, cargo-release:

1. ✅ Updates version in `Cargo.toml`
2. ✅ Updates `CHANGELOG.md` with version and date
3. ✅ Commits the changes
4. ✅ Creates a git tag (e.g., `v0.1.1`)
5. ✅ Pushes commits and tags to GitHub

## Maintaining the Changelog

Before releasing, ensure `CHANGELOG.md` has an `[Unreleased]` section with your changes:

```markdown
## [Unreleased]

### Added
- New feature description

### Changed
- Changed feature description

### Fixed
- Bug fix description
```

cargo-release will automatically:
- Move `[Unreleased]` content to a new versioned section
- Add the version number and date
- Leave an empty `[Unreleased]` section for future changes

## Example Workflow

```bash
# 1. Make code changes
git checkout -b feature/new-feature

# 2. Update CHANGELOG.md under [Unreleased]
# Add your changes to CHANGELOG.md

# 3. Commit and merge to main
git add .
git commit -m "Add new feature"
git push
# Create PR, merge to main

# 4. Preview the release
cargo release patch

# 5. Execute the release
cargo release patch --execute

# 6. Verify on GitHub
# Check: https://github.com/wingnut128/prate/releases
# Check: https://github.com/wingnut128/prate/tags
```

## Troubleshooting

### Uncommitted Changes

If you see "uncommitted changes detected":
```bash
git status
git add .
git commit -m "Your commit message"
```

### Wrong Version Bump

If you executed the wrong version bump:
```bash
# Delete the local tag
git tag -d v0.1.1

# Delete the remote tag
git push origin :refs/tags/v0.1.1

# Reset to previous commit
git reset --hard HEAD~1

# Try again with correct version
cargo release minor --execute
```

### Dry Run is Default

Note: `cargo release` runs in dry-run mode by default. You must add `--execute` to actually perform the release.

## Configuration

Release configuration is in `Cargo.toml` under `[package.metadata.release]`:

```toml
[package.metadata.release]
publish = false  # Don't publish to crates.io
push = true      # Push to git remote
tag = true       # Create git tags
tag-prefix = "v" # Tag format: v0.1.0
sign-tag = false # Don't sign tags (can enable if you have GPG)
```

## GitHub Releases

After cargo-release creates the tag, you can create a GitHub Release manually:

1. Go to https://github.com/wingnut128/prate/releases/new
2. Select the tag (e.g., `v0.1.1`)
3. Title: `v0.1.1`
4. Copy the relevant section from `CHANGELOG.md` into the description
5. Publish release

Alternatively, this can be automated with GitHub Actions in the future.
