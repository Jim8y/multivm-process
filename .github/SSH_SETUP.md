# SSH Key Setup for Private Repository Access

This document explains how to configure SSH keys for GitHub Actions to access private repositories.

## Overview

All GitHub Actions workflows in this repository have been configured to use SSH keys for accessing private repositories. This is accomplished using the `webfactory/ssh-agent` action and a repository secret named `SSH_KEY`.

## Setup Instructions

### 1. Generate SSH Key Pair

Generate a new SSH key pair specifically for GitHub Actions:

```bash
ssh-keygen -t ed25519 -C "github-actions@your-repo" -f ~/.ssh/github_actions_key
```

This creates:
- `~/.ssh/github_actions_key` (private key)
- `~/.ssh/github_actions_key.pub` (public key)

### 2. Add Public Key to GitHub

1. Copy the public key content:
   ```bash
   cat ~/.ssh/github_actions_key.pub
   ```

2. Add it to your GitHub account or organization:
   - **For personal repositories**: Go to GitHub Settings → SSH and GPG keys → New SSH key
   - **For organization repositories**: Go to Organization Settings → SSH and GPG keys → New SSH key
   - **For specific repository access**: Add as a deploy key in the target repository settings

### 3. Add Private Key as Repository Secret

1. Copy the private key content:
   ```bash
   cat ~/.ssh/github_actions_key
   ```

2. Add it as a repository secret:
   - Go to your repository → Settings → Secrets and variables → Actions
   - Click "New repository secret"
   - Name: `SSH_KEY`
   - Value: Paste the entire private key content (including `-----BEGIN` and `-----END` lines)

## How It Works

### SSH Agent Setup

Each workflow job includes this step before checkout:

```yaml
- name: Setup SSH key
  uses: webfactory/ssh-agent@v0.9.0
  with:
    ssh-private-key: ${{ secrets.SSH_KEY }}
```

This action:
1. Starts an SSH agent
2. Adds the private key to the agent
3. Configures Git to use SSH for GitHub operations
4. Sets up known hosts for github.com

### Automatic Git Configuration

The `webfactory/ssh-agent` action automatically:
- Configures Git to use SSH instead of HTTPS for GitHub
- Adds github.com to known hosts
- Sets up the SSH agent for the duration of the job

## Accessing Private Repositories

### In Cargo.toml Dependencies

For Rust projects, you can now reference private repositories in `Cargo.toml`:

```toml
[dependencies]
my-private-crate = { git = "git@github.com:your-org/private-repo.git" }
```

### In Workflow Steps

You can clone additional private repositories:

```yaml
- name: Clone private dependency
  run: git clone git@github.com:your-org/private-repo.git
```

### Submodules

If your repository uses private submodules, they will be automatically accessible:

```yaml
- name: Checkout code with submodules
  uses: actions/checkout@v4
  with:
    submodules: recursive
```

## Security Considerations

### Key Scope

- **Repository-specific keys**: Create separate SSH keys for each repository/project
- **Minimal permissions**: Use deploy keys with read-only access when possible
- **Key rotation**: Regularly rotate SSH keys (recommended: every 6-12 months)

### Secret Management

- Never commit private keys to the repository
- Use repository secrets, not environment variables in workflow files
- Limit secret access to necessary workflows only

### Access Control

- Use organization-level secrets for keys that need access across multiple repositories
- Consider using GitHub Apps with fine-grained permissions for more complex scenarios

## Troubleshooting

### Common Issues

1. **Permission denied (publickey)**
   - Verify the private key is correctly added to secrets
   - Ensure the public key is added to the correct GitHub account/organization
   - Check that the key format is correct (including newlines)

2. **Host key verification failed**
   - The `webfactory/ssh-agent` action should handle this automatically
   - If issues persist, add a manual known_hosts setup

3. **Git operations still use HTTPS**
   - Ensure repository URLs use SSH format (`git@github.com:...`)
   - The ssh-agent action should automatically configure Git URL rewriting

### Debug Steps

Add this step to debug SSH configuration:

```yaml
- name: Debug SSH setup
  run: |
    ssh -T git@github.com
    git config --list | grep url
```

## Workflow Coverage

The following workflows have been updated with SSH key support:

- **CI** (`.github/workflows/ci.yml`): All jobs
- **Maintenance** (`.github/workflows/maintenance.yml`): All jobs  
- **PR Checks** (`.github/workflows/pr-checks.yml`): All jobs
- **Release** (`.github/workflows/release.yml`): All jobs

## Best Practices

1. **Use dedicated keys**: Create separate SSH keys for CI/CD, don't reuse personal keys
2. **Minimal scope**: Grant only the minimum required access
3. **Monitor usage**: Regularly review SSH key usage in GitHub audit logs
4. **Document access**: Keep track of which keys have access to which repositories
5. **Automate rotation**: Consider automating SSH key rotation for production environments

## Alternative Approaches

### GitHub App Authentication

For more complex scenarios, consider using GitHub Apps:

```yaml
- name: Generate token
  id: generate_token
  uses: tibdex/github-app-token@v1
  with:
    app_id: ${{ secrets.APP_ID }}
    private_key: ${{ secrets.APP_PRIVATE_KEY }}

- name: Checkout with app token
  uses: actions/checkout@v4
  with:
    token: ${{ steps.generate_token.outputs.token }}
```

### Personal Access Tokens

For simpler cases, you might use PATs (though SSH keys are generally preferred):

```yaml
- name: Checkout with PAT
  uses: actions/checkout@v4
  with:
    token: ${{ secrets.GITHUB_TOKEN }}
```

## Support

If you encounter issues with SSH key setup:

1. Check the GitHub Actions logs for specific error messages
2. Verify the SSH key is correctly formatted in the secret
3. Ensure the public key is added to the correct GitHub account
4. Test SSH access manually: `ssh -T git@github.com`