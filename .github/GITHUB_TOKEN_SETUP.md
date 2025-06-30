# GitHub Token Configuration Guide

This guide explains how to configure GitHub token access for your workflows to access private repositories within your organization.

## Understanding GITHUB_TOKEN

The `GITHUB_TOKEN` is **automatically provided** by GitHub Actions - you don't need to create it manually. However, you need to configure permissions at the organization and repository level.

## Step-by-Step Configuration

### 1. Organization-Level Settings

#### A. Enable Actions for Organization
1. Go to your **Organization Settings**
2. Navigate to **Actions** → **General**
3. Under "Actions permissions", select:
   - ✅ **"Allow all actions and reusable workflows"** (recommended)
   - OR ✅ **"Allow select actions and reusable workflows"** (more restrictive)

#### B. Configure Workflow Permissions
1. In **Organization Settings** → **Actions** → **General**
2. Scroll to **"Workflow permissions"**
3. Choose one of:
   - ✅ **"Read and write permissions"** (gives workflows broad access)
   - ✅ **"Read repository contents and packages permissions"** (more secure, recommended)

#### C. Allow Actions to Access Organization Repositories
1. In **Organization Settings** → **Actions** → **General**
2. Under **"Actions permissions"**, ensure:
   - ✅ **"Allow actions created by GitHub"** is checked
   - ✅ **"Allow actions by Marketplace verified creators"** is checked

### 2. Repository-Level Settings

#### A. Enable Actions for This Repository
1. Go to your **Repository Settings**
2. Navigate to **Actions** → **General**
3. Under "Actions permissions", select:
   - ✅ **"Allow all actions and reusable workflows"**

#### B. Configure Workflow Permissions for Repository
1. In **Repository Settings** → **Actions** → **General**
2. Scroll to **"Workflow permissions"**
3. Choose:
   - ✅ **"Read repository contents and packages permissions"** (recommended)
   - ✅ Check **"Allow GitHub Actions to create and approve pull requests"** if needed

### 3. Target Private Repository Settings

For each private repository you want to access:

#### A. Enable Actions Access
1. Go to the **private repository** you want to access
2. Navigate to **Settings** → **Actions** → **General**
3. Under "Actions permissions", ensure it's not set to "Disable actions"

#### B. Check Repository Visibility
1. Ensure the repository is **within the same organization**
2. The repository should be **private** (not internal/public if you need special access)

### 4. Package/Container Registry Settings (If Using)

If you're accessing private packages:

#### A. Organization Package Settings
1. Go to **Organization Settings** → **Packages**
2. Configure package visibility and permissions
3. Ensure Actions can access packages

#### B. Package Permissions
1. For each private package, check its permissions
2. Ensure your organization/repository has access

## Verification Steps

### 1. Test Basic Access

Add this step to any workflow to test token permissions:

```yaml
- name: Test GitHub Token Access
  run: |
    echo "Testing GitHub token permissions..."
    
    # Test API access
    curl -H "Authorization: token ${{ secrets.GITHUB_TOKEN }}" \
         -H "Accept: application/vnd.github.v3+json" \
         https://api.github.com/user
    
    # Test repository access
    curl -H "Authorization: token ${{ secrets.GITHUB_TOKEN }}" \
         -H "Accept: application/vnd.github.v3+json" \
         https://api.github.com/repos/${{ github.repository }}
```

### 2. Test Private Repository Access

```yaml
- name: Test Private Repo Access
  run: |
    # Replace 'your-org/private-repo' with actual private repo
    git clone https://x-access-token:${{ secrets.GITHUB_TOKEN }}@github.com/your-org/private-repo.git
```

### 3. Test Package Access

```yaml
- name: Test Package Access
  run: |
    # This will test if Cargo can access private dependencies
    cargo fetch --verbose
```

## Common Configuration Issues

### Issue 1: "Resource not accessible by integration"

**Cause**: Insufficient workflow permissions

**Solution**:
1. Check organization workflow permissions
2. Ensure repository allows Actions
3. Verify the workflow has correct `permissions:` block

```yaml
permissions:
  contents: read
  packages: read
```

### Issue 2: "Repository not found"

**Cause**: Repository is not accessible or doesn't exist

**Solution**:
1. Verify repository is in the same organization
2. Check repository name spelling
3. Ensure repository exists and is private

### Issue 3: "Authentication failed"

**Cause**: Token doesn't have required permissions

**Solution**:
1. Check organization settings allow Actions
2. Verify repository settings allow Actions
3. Ensure workflow permissions are correctly set

## Advanced Configuration

### Using Organization Secrets

For shared configuration across repositories:

1. Go to **Organization Settings** → **Secrets and variables** → **Actions**
2. Add organization-level secrets
3. Configure repository access for secrets

### Custom GitHub App (Advanced)

For more complex scenarios, create a GitHub App:

1. Go to **Organization Settings** → **Developer settings** → **GitHub Apps**
2. Create new GitHub App with required permissions
3. Install the app on target repositories
4. Use app credentials in workflows:

```yaml
- name: Generate App Token
  id: generate_token
  uses: tibdex/github-app-token@v1
  with:
    app_id: ${{ secrets.APP_ID }}
    private_key: ${{ secrets.APP_PRIVATE_KEY }}
```

## Quick Checklist

Use this checklist to ensure everything is configured:

### Organization Level:
- [ ] Actions enabled for organization
- [ ] Workflow permissions set to "Read repository contents and packages"
- [ ] Actions can access organization repositories

### Repository Level:
- [ ] Actions enabled for this repository
- [ ] Workflow permissions configured
- [ ] Repository is in the same organization as target private repos

### Target Private Repositories:
- [ ] Actions not disabled
- [ ] Repository is accessible within organization
- [ ] Package permissions configured (if applicable)

### Workflow Files:
- [ ] `permissions:` block added to each workflow
- [ ] `token: ${{ secrets.GITHUB_TOKEN }}` added to checkout actions
- [ ] Private repository URLs use HTTPS format

## Testing Your Configuration

Create a simple test workflow to verify everything works:

```yaml
name: Test Private Access
on:
  workflow_dispatch:

permissions:
  contents: read
  packages: read

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
    - name: Checkout
      uses: actions/checkout@v4
      with:
        token: ${{ secrets.GITHUB_TOKEN }}
    
    - name: Test API Access
      run: |
        curl -H "Authorization: token ${{ secrets.GITHUB_TOKEN }}" \
             https://api.github.com/user
    
    - name: Test Private Repo Clone
      run: |
        # Replace with your actual private repo
        git clone https://x-access-token:${{ secrets.GITHUB_TOKEN }}@github.com/your-org/your-private-repo.git
```

## Summary

The key points for GitHub token configuration:

1. **GITHUB_TOKEN is automatic** - no manual creation needed
2. **Configure organization settings** - enable Actions and set permissions
3. **Configure repository settings** - allow Actions access
4. **Add permissions to workflows** - specify what each workflow needs
5. **Use HTTPS URLs** - for Git operations with the token
6. **Test thoroughly** - verify access works as expected

Once configured, your workflows will automatically have access to private repositories in your organization without any additional secrets or setup!