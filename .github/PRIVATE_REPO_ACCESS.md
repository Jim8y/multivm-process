# Private Repository Access for GitHub Actions

This document explains how GitHub Actions workflows are configured to access private repositories within the same organization using GitHub's standard authentication methods.

## Overview

All GitHub Actions workflows in this repository use GitHub's built-in `GITHUB_TOKEN` with proper permissions to access private repositories within the same organization. This is the recommended and most standard approach for organization-level private repository access.

## How It Works

### Built-in GITHUB_TOKEN

Each workflow uses the automatically provided `GITHUB_TOKEN`:

```yaml
- name: Checkout code
  uses: actions/checkout@v4
  with:
    token: ${{ secrets.GITHUB_TOKEN }}
```

### Workflow Permissions

Each workflow declares the necessary permissions:

```yaml
permissions:
  contents: read      # Read repository contents
  packages: read      # Read packages (for private dependencies)
  pull-requests: read # Read PR information (PR workflows only)
  contents: write     # Write access (release workflow only)
```

## Current Configuration

### CI Workflow (`.github/workflows/ci.yml`)
- **Permissions**: `contents: read`, `packages: read`
- **Access**: Can read private repos and packages in the organization
- **Jobs**: All 5 jobs configured with GITHUB_TOKEN

### Maintenance Workflow (`.github/workflows/maintenance.yml`)
- **Permissions**: `contents: read`, `packages: read`
- **Access**: Can read private repos and packages for dependency auditing
- **Jobs**: All 3 jobs configured with GITHUB_TOKEN

### PR Checks Workflow (`.github/workflows/pr-checks.yml`)
- **Permissions**: `contents: read`, `packages: read`, `pull-requests: read`
- **Access**: Can read private repos, packages, and PR information
- **Jobs**: All 3 jobs configured with GITHUB_TOKEN

### Release Workflow (`.github/workflows/release.yml`)
- **Permissions**: `contents: write`, `packages: read`
- **Access**: Can read private repos/packages and create releases
- **Jobs**: All 3 jobs configured with GITHUB_TOKEN

## Accessing Private Dependencies

### Cargo.toml Dependencies

For Rust projects, you can reference private repositories in your organization:

```toml
[dependencies]
# Using HTTPS (recommended for GITHUB_TOKEN)
my-private-crate = { git = "https://github.com/your-org/private-repo.git" }

# With specific branch/tag
my-private-crate = { git = "https://github.com/your-org/private-repo.git", branch = "main" }

# With specific commit
my-private-crate = { git = "https://github.com/your-org/private-repo.git", rev = "abc123" }
```

### Git Operations in Workflows

You can clone additional private repositories:

```yaml
- name: Clone private dependency
  run: |
    git clone https://x-access-token:${{ secrets.GITHUB_TOKEN }}@github.com/your-org/private-repo.git
```

### Submodules

Private submodules work automatically with the configured token:

```yaml
- name: Checkout with submodules
  uses: actions/checkout@v4
  with:
    token: ${{ secrets.GITHUB_TOKEN }}
    submodules: recursive
```

## Advantages of This Approach

### 1. **No Setup Required**
- `GITHUB_TOKEN` is automatically provided by GitHub
- No need to create or manage additional secrets
- Works immediately for organization repositories

### 2. **Secure by Default**
- Token is automatically scoped to the current repository and organization
- Permissions are explicitly declared and minimal
- Token expires after the workflow run

### 3. **Standard Practice**
- This is GitHub's recommended approach for organization access
- Widely documented and supported
- Consistent with GitHub's security model

### 4. **Automatic Scope Management**
- Token automatically has access to:
  - The current repository
  - Other repositories in the same organization (with proper permissions)
  - Organization packages and container registry

## Limitations and Alternatives

### GITHUB_TOKEN Limitations

The `GITHUB_TOKEN` has some limitations:
- Cannot trigger other workflows (prevents recursive workflow runs)
- Limited to the current organization
- Cannot access personal repositories of organization members

### Alternative: GitHub App (For Complex Scenarios)

For more complex scenarios, you can use a GitHub App:

```yaml
- name: Generate App Token
  id: generate_token
  uses: tibdex/github-app-token@v1
  with:
    app_id: ${{ secrets.APP_ID }}
    private_key: ${{ secrets.APP_PRIVATE_KEY }}
    repository: your-org/target-repo

- name: Checkout with App Token
  uses: actions/checkout@v4
  with:
    token: ${{ steps.generate_token.outputs.token }}
```

### Alternative: Personal Access Token (Not Recommended)

For cross-organization access (not recommended for security reasons):

```yaml
- name: Checkout with PAT
  uses: actions/checkout@v4
  with:
    token: ${{ secrets.PERSONAL_ACCESS_TOKEN }}
```

## Organization Settings

### Required Organization Settings

Ensure your organization has the following settings configured:

1. **Actions Permissions**:
   - Go to Organization Settings → Actions → General
   - Allow actions to access repositories in the organization

2. **Package Permissions**:
   - Go to Organization Settings → Packages
   - Configure package visibility and access permissions

3. **Repository Access**:
   - Individual repositories should allow Actions access
   - Check repository Settings → Actions → General

### Workflow Permissions

You can also configure default permissions at the organization level:
- Organization Settings → Actions → General → Workflow permissions
- Choose "Read repository contents and packages permissions" or more restrictive

## Troubleshooting

### Common Issues

1. **Permission Denied**
   ```
   Error: Resource not accessible by integration
   ```
   **Solution**: Check workflow permissions and organization settings

2. **Private Repository Not Found**
   ```
   Error: Repository not found
   ```
   **Solution**: Ensure the repository is in the same organization and accessible

3. **Package Access Denied**
   ```
   Error: Package not found or access denied
   ```
   **Solution**: Check package visibility and organization package permissions

### Debug Steps

Add this step to debug token permissions:

```yaml
- name: Debug Token Permissions
  run: |
    curl -H "Authorization: token ${{ secrets.GITHUB_TOKEN }}" \
         -H "Accept: application/vnd.github.v3+json" \
         https://api.github.com/user
```

## Best Practices

### 1. **Minimal Permissions**
- Only grant the minimum required permissions
- Use `read` permissions unless `write` is specifically needed

### 2. **Explicit Permission Declaration**
- Always declare permissions explicitly in workflows
- Don't rely on default permissions

### 3. **Organization-Level Configuration**
- Configure organization settings to support Actions
- Use organization secrets for shared configuration

### 4. **Repository URLs**
- Use HTTPS URLs for Git operations with GITHUB_TOKEN
- Format: `https://github.com/org/repo.git`

### 5. **Error Handling**
- Add proper error handling for private repository access
- Use `continue-on-error: true` for non-critical operations

## Migration from SSH Keys

If you were previously using SSH keys, the migration is straightforward:

### Before (SSH):
```yaml
- name: Setup SSH key
  uses: webfactory/ssh-agent@v0.9.0
  with:
    ssh-private-key: ${{ secrets.SSH_KEY }}

- name: Checkout
  uses: actions/checkout@v4
```

### After (GITHUB_TOKEN):
```yaml
- name: Checkout
  uses: actions/checkout@v4
  with:
    token: ${{ secrets.GITHUB_TOKEN }}
```

### Update Cargo.toml:
```toml
# Change from SSH format
my-crate = { git = "git@github.com:org/repo.git" }

# To HTTPS format
my-crate = { git = "https://github.com/org/repo.git" }
```

## Summary

This standard approach using `GITHUB_TOKEN` provides:
- ✅ Zero setup required
- ✅ Secure by default
- ✅ Organization-wide access
- ✅ GitHub's recommended practice
- ✅ Automatic permission management
- ✅ No additional secrets to manage

For most organization use cases, this is the preferred and most maintainable solution.