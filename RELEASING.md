# Release Process

Releases are created manually using the GitHub CLI.

## Semantic Versioning

This project follows [Semantic Versioning](https://semver.org/):
- **PATCH** (0.5.1 → 0.5.2): Bug fixes, minor changes
- **MINOR** (0.5.1 → 0.6.0): New features, backward compatible
- **MAJOR** (0.5.1 → 1.0.0): Breaking changes

## Release Steps

### 1. Bump the version

Update `Cargo.toml` with the new version number and push via a PR:

```bash
# Edit Cargo.toml version field
# Run cargo check to update Cargo.lock
cargo check

# Commit both files via a feature branch + PR
```

### 2. Update the changelog

Add a new section to `CHANGELOG.md` under `[Unreleased]`:

```markdown
## [0.6.0] - 2026-03-15

### Added
- New feature description

### Changed
- Changed feature description

### Fixed
- Bug fix description
```

This can be in the same PR as the version bump.

### 3. Create the GitHub release

After the PR is merged to main:

```bash
gh release create v0.6.0 \
  --target main \
  --title "outlier v0.6.0" \
  --notes "Release notes here..."
```

### 4. Verify

```bash
# Check the release
gh release view v0.6.0

# Check tags
git fetch --tags
git tag -l
```

## Troubleshooting

### Wrong version released

If you tagged the wrong version:

```bash
# Delete the GitHub release
gh release delete v0.6.0 --yes

# Delete the remote tag
git push origin :refs/tags/v0.6.0

# Delete the local tag
git tag -d v0.6.0
```

Then fix the version in a new PR and re-release.
