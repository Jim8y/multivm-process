# Using Private Repository Dependencies with Cargo

This guide shows how to configure your Rust project to use private GitHub repositories as dependencies with GitHub Actions authentication.

## How It Works

Your workflows are now configured with Git URL rewriting that automatically adds authentication tokens to GitHub URLs:

```bash
git config --global url."https://x-access-token:${{ secrets.GITHUB_TOKEN }}@github.com/".insteadOf "https://github.com/"
```

This means when Cargo tries to fetch a dependency from `https://github.com/your-org/private-repo.git`, it automatically becomes `https://x-access-token:TOKEN@github.com/your-org/private-repo.git`.

## Adding Private Dependencies

### 1. Workspace-Level Dependencies (Recommended)

Add private dependencies to your main `Cargo.toml` workspace dependencies:

```toml
[workspace.dependencies]
# Your existing dependencies...

# Private repository dependencies
my-private-utils = { git = "https://github.com/your-org/private-utils.git" }
shared-types = { git = "https://github.com/your-org/shared-types.git", branch = "main" }
crypto-lib = { git = "https://github.com/your-org/crypto-lib.git", tag = "v1.2.0" }
experimental-feature = { git = "https://github.com/your-org/experimental.git", rev = "abc123def" }

# Private dependencies with features
advanced-networking = { git = "https://github.com/your-org/networking.git", features = ["async", "tls"] }

# Private dependencies with specific versions
database-connector = { git = "https://github.com/your-org/db-connector.git", branch = "stable" }
```

### 2. Crate-Level Dependencies

Then use them in individual crates like `multivm-common/Cargo.toml`:

```toml
[dependencies]
# Existing dependencies...
multivm-common = { workspace = true }

# Private dependencies from workspace
my-private-utils = { workspace = true }
shared-types = { workspace = true }

# Or directly in the crate (not recommended for workspace projects)
another-private-lib = { git = "https://github.com/your-org/another-lib.git" }
```

## Example: Adding a Private Dependency

Let's say you want to add a private cryptography library to your project:

### Step 1: Add to Workspace Dependencies

Edit your main `Cargo.toml`:

```toml
[workspace.dependencies]
# ... existing dependencies ...

# Add your private dependency
multivm-crypto-utils = { git = "https://github.com/your-org/multivm-crypto-utils.git", branch = "main" }
```

### Step 2: Use in Specific Crates

Edit `multivm-common/Cargo.toml`:

```toml
[dependencies]
# ... existing dependencies ...

# Add the private dependency
multivm-crypto-utils = { workspace = true }
```

### Step 3: Use in Code

In your Rust code (`multivm-common/src/lib.rs`):

```rust
// Import from your private dependency
use multivm_crypto_utils::{encrypt, decrypt, KeyPair};

pub fn secure_operation(data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let keypair = KeyPair::generate()?;
    let encrypted = encrypt(data, &keypair.public_key())?;
    Ok(encrypted)
}
```

## Dependency Specification Options

### Branch-based Dependencies
```toml
my-lib = { git = "https://github.com/your-org/my-lib.git", branch = "develop" }
```

### Tag-based Dependencies
```toml
my-lib = { git = "https://github.com/your-org/my-lib.git", tag = "v2.1.0" }
```

### Commit-based Dependencies
```toml
my-lib = { git = "https://github.com/your-org/my-lib.git", rev = "a1b2c3d4e5f6" }
```

### With Features
```toml
my-lib = { git = "https://github.com/your-org/my-lib.git", features = ["serde", "async"] }
```

### With Default Features Disabled
```toml
my-lib = { git = "https://github.com/your-org/my-lib.git", default-features = false, features = ["minimal"] }
```

## Best Practices

### 1. Use Workspace Dependencies
Always add private dependencies to the workspace `[workspace.dependencies]` section first, then reference them with `{ workspace = true }` in individual crates.

### 2. Pin to Specific Versions
For production, use specific tags or commit hashes rather than branches:

```toml
# Good for production
stable-lib = { git = "https://github.com/your-org/stable-lib.git", tag = "v1.0.0" }

# Good for development
dev-lib = { git = "https://github.com/your-org/dev-lib.git", branch = "main" }
```

### 3. Document Private Dependencies
Add comments explaining what each private dependency does:

```toml
[workspace.dependencies]
# Internal cryptography utilities for MultiVM
multivm-crypto = { git = "https://github.com/your-org/multivm-crypto.git", tag = "v2.1.0" }

# Shared data structures across MultiVM components  
multivm-types = { git = "https://github.com/your-org/multivm-types.git", branch = "main" }
```

### 4. Use Cargo.lock for Reproducible Builds
Commit your `Cargo.lock` file to ensure reproducible builds across environments.

## Troubleshooting

### Common Issues

#### 1. "Couldn't find repository" Error
```
error: failed to get `my-private-lib` as a dependency of package `my-crate`
Caused by: failed to load source for dependency `my-private-lib`
Caused by: Unable to update https://github.com/your-org/my-private-lib.git
Caused by: couldn't find repository from 'https://github.com/your-org/my-private-lib.git'
```

**Solutions:**
- Verify the repository URL is correct
- Ensure the repository exists and is accessible
- Check that your GitHub token has access to the organization
- Verify the repository is in the same organization

#### 2. "Permission denied" Error
```
error: failed to get `my-private-lib` as a dependency of package `my-crate`
Caused by: failed to load source for dependency `my-private-lib`
Caused by: Unable to update https://github.com/your-org/my-private-lib.git
Caused by: authentication required
```

**Solutions:**
- Check that the Git URL rewriting is configured in your workflow
- Verify your GitHub token has the correct permissions
- Ensure the workflow has `contents: read` and `packages: read` permissions

#### 3. "Branch/Tag not found" Error
```
error: failed to get `my-private-lib` as a dependency of package `my-crate`
Caused by: failed to load source for dependency `my-private-lib`
Caused by: Unable to update https://github.com/your-org/my-private-lib.git
Caused by: revspec 'v1.0.0' not found
```

**Solutions:**
- Verify the branch/tag/commit exists in the repository
- Check the exact spelling of branch/tag names
- Use `git ls-remote` to list available refs

### Debug Commands

Add these steps to your workflow for debugging:

```yaml
- name: Debug Git Configuration
  run: |
    git config --list | grep url
    echo "Testing repository access..."
    git ls-remote https://github.com/your-org/your-private-repo.git

- name: Debug Cargo Dependencies
  run: |
    cargo tree --verbose
    cargo fetch --verbose
```

## Local Development

For local development, you have several options:

### Option 1: Use Personal Access Token
Create a `.cargo/config.toml` file in your project root:

```toml
[net]
git-fetch-with-cli = true

[http]
debug = false
```

Then configure Git globally:
```bash
git config --global url."https://YOUR_GITHUB_TOKEN@github.com/".insteadOf "https://github.com/"
```

### Option 2: Use SSH Keys Locally
Configure your local Git to use SSH:
```bash
git config --global url."git@github.com:".insteadOf "https://github.com/"
```

And ensure your SSH key has access to the private repositories.

### Option 3: Use GitHub CLI
```bash
gh auth login
# This will configure Git authentication automatically
```

## Security Considerations

### 1. Repository Access
- Only grant access to repositories that actually need the private dependencies
- Use organization-level access control
- Regularly audit which repositories have access to private dependencies

### 2. Token Scope
- The `GITHUB_TOKEN` is automatically scoped to your organization
- It only has access during workflow execution
- No additional token management required

### 3. Dependency Auditing
Your maintenance workflow will automatically audit private dependencies:
- Security vulnerabilities
- License compliance
- Outdated versions

## Example: Complete Setup

Here's a complete example of adding a private dependency:

### 1. Main Cargo.toml
```toml
[workspace.dependencies]
# ... existing dependencies ...

# Private shared utilities
multivm-shared-utils = { git = "https://github.com/your-org/multivm-shared-utils.git", tag = "v1.0.0" }
```

### 2. multivm-common/Cargo.toml
```toml
[dependencies]
# ... existing dependencies ...

# Use the private dependency
multivm-shared-utils = { workspace = true }
```

### 3. Use in Code
```rust
// multivm-common/src/utils.rs
use multivm_shared_utils::common_function;

pub fn enhanced_operation() -> Result<(), Box<dyn std::error::Error>> {
    let result = common_function()?;
    // Your logic here
    Ok(())
}
```

### 4. Test the Setup
Run your test workflow:
```bash
# In GitHub Actions, this will automatically work
cargo build
cargo test
```

## Migration from SSH

If you're migrating from SSH-based dependencies:

### Before (SSH):
```toml
my-lib = { git = "git@github.com:your-org/my-lib.git" }
```

### After (HTTPS):
```toml
my-lib = { git = "https://github.com/your-org/my-lib.git" }
```

The workflows will automatically handle authentication for HTTPS URLs.

## Summary

With this setup:
- ✅ Your workflows can access private repositories automatically
- ✅ Cargo can fetch private dependencies during builds
- ✅ No manual token management required
- ✅ Secure by default with proper scoping
- ✅ Works for all organization repositories
- ✅ Supports all Cargo dependency features (branches, tags, commits, features)

Your GitHub Actions workflows are now fully configured to work with private repository dependencies!