# NPM Token Setup Guide

## Quick Setup

### 1. Generate NPM Access Token

1. Go to https://www.npmjs.com/settings/[your-username]/tokens
2. Click **"Generate New Token"** → **"Classic Token"**
3. Select **"Automation"** type (for CI/CD)
4. Copy the generated token (starts with `npm_...`)

### 2. Add Token to GitHub Repository

1. Go to your repository: https://github.com/ruvnet/ruvector
2. Navigate to **Settings** → **Secrets and variables** → **Actions**
3. Click **"New repository secret"**
4. Name: `NPM_TOKEN`
5. Value: Paste your npm token
6. Click **"Add secret"**

### 3. Verify Token Works

After adding the secret, re-run the publishing workflow:

```bash
# Delete and recreate the tag to trigger workflow again
git tag -d v0.1.2
git push origin :refs/tags/v0.1.2

# Create and push tag again
git tag v0.1.2 -a -m "Release v0.1.2"
git push origin v0.1.2
```

Or manually trigger the workflow:

```bash
gh workflow run "Build Native Modules"
```

## Detailed Instructions

### Creating NPM Access Token

#### Requirements
- NPM account with publish permissions
- Member of `@ruvector` organization (if scoped package)

#### Token Types

**Automation Token (Recommended for CI/CD)**
- ✅ No IP restrictions
- ✅ Works in GitHub Actions
- ✅ Can publish packages
- ⚠️ Never expires (revoke if compromised)

**Granular Access Token (More Secure)**
- ✅ Can set expiration
- ✅ Can limit to specific packages
- ✅ Can restrict IPs
- ⚠️ May require re-authentication

#### Steps

1. **Login to NPM**
   ```bash
   npm login
   ```

2. **Navigate to Token Settings**
   - Visit: https://www.npmjs.com/settings/[username]/tokens
   - Or: NPM profile → Access Tokens

3. **Generate New Token**
   - Click "Generate New Token"
   - Choose "Classic Token"
   - Select "Automation" type
   - Optionally set:
     - Token name/description
     - IP allowlist (leave empty for GitHub Actions)
     - CIDR ranges if needed

4. **Copy Token**
   - Token format: `npm_xxxxxxxxxxxxxxxxxxxxxxxxxx`
   - ⚠️ **Save immediately** - shown only once
   - Store securely (password manager recommended)

### Adding to GitHub Repository

#### Via Web Interface

1. **Navigate to Repository Settings**
   ```
   https://github.com/ruvnet/ruvector/settings/secrets/actions
   ```

2. **Add New Secret**
   - Click **"New repository secret"**
   - Name: `NPM_TOKEN` (exact name required)
   - Value: Your npm token
   - Click **"Add secret"**

3. **Verify Secret Added**
   - Secret should appear in list
   - Value is masked (••••)
   - Can update but not view

#### Via GitHub CLI

```bash
# Set repository secret
gh secret set NPM_TOKEN --body "npm_your_token_here"

# Verify secret exists
gh secret list
```

### Testing Authentication

#### Test Locally (Optional)

```bash
# Test token works
echo "//registry.npmjs.com/:_authToken=\${NPM_TOKEN}" > .npmrc
export NPM_TOKEN="your_token_here"
npm whoami
```

#### Test in GitHub Actions

Create test workflow or check existing run:

```bash
# View latest workflow run
gh run list --limit 1

# Check for authentication errors
gh run view --log | grep -i "auth\|token\|login"
```

## Troubleshooting

### Token Not Working

**Symptom:**
```
npm error code ENEEDAUTH
npm error need auth
```

**Checks:**
1. ✅ Secret name is exactly `NPM_TOKEN`
2. ✅ Token starts with `npm_`
3. ✅ Token type is "Automation" or "Publish"
4. ✅ Token hasn't expired
5. ✅ Account has publish permissions

**Solutions:**
- Regenerate token
- Check token permissions
- Verify organization access
- Check IP restrictions

### Permission Denied

**Symptom:**
```
npm error code E403
npm error 403 Forbidden
```

**Checks:**
1. ✅ Have publish permissions for package
2. ✅ Member of `@ruvector` org (if scoped)
3. ✅ Package name not taken
4. ✅ Not blocked by npm

**Solutions:**
```bash
# Check package ownership
npm owner ls @ruvector/core

# Add yourself as owner
npm owner add <username> @ruvector/core

# Check if package exists
npm view @ruvector/core
```

### Token Expired

**Symptom:**
```
npm error code EAUTHIP
npm error Unable to authenticate
```

**Solution:**
1. Generate new token
2. Update GitHub secret
3. Re-run workflow

### Wrong Package Directory

**Symptom:**
```
npm error Cannot find package.json
npm error code ENOENT
```

**Solution:**
Check workflow working directory:
```yaml
- name: Publish main package
  working-directory: npm/packages/ruvector  # ← Verify correct path
  run: npm publish --access public
```

## Security Best Practices

### Token Security

- ✅ Never commit tokens to git
- ✅ Use automation tokens for CI/CD
- ✅ Rotate tokens periodically
- ✅ Revoke compromised tokens immediately
- ✅ Use granular tokens when possible
- ✅ Set IP restrictions if feasible
- ✅ Monitor npm audit logs

### GitHub Secrets

- ✅ Use repository secrets (not environment)
- ✅ Limit who can view/edit secrets
- ✅ Use environments for staging/prod
- ✅ Enable branch protection
- ✅ Require approvals for deployments
- ✅ Audit secret access logs

### Additional Security

```yaml
# Use environment protection
publish:
  environment:
    name: npm-production
    url: https://www.npmjs.com/package/@ruvector/core
  needs: build
```

## Alternative: Manual Publishing

If you prefer not to use automated publishing:

```bash
# 1. Build all platforms locally
npm run build

# 2. Login to npm
npm login

# 3. Publish manually
cd npm/packages/ruvector
npm publish --access public
```

## Verification

After adding token:

```bash
# 1. Trigger new build
git tag v0.1.2 -f
git push origin v0.1.2 -f

# 2. Monitor workflow
gh run watch

# 3. Verify publication
npm view @ruvector/core versions
```

## Support

If issues persist:
- Check [NPM Documentation](https://docs.npmjs.com/creating-and-viewing-access-tokens)
- Review [GitHub Secrets Docs](https://docs.github.com/en/actions/security-guides/encrypted-secrets)
- Contact repository administrators

---

**Important:** Keep your NPM token secure and never share it publicly!
