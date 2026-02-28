# Security Best Practices for Ruvector Development

## Environment Variables and Secrets

### Never Commit Secrets

**Critical**: Never commit API keys, tokens, or credentials to version control.

### Protected Files

The following files are in `.gitignore` and should **NEVER** be committed:

```
.env                    # Main environment configuration
.env.local             # Local overrides
.env.*.local           # Environment-specific local configs
*.key                  # Private keys
*.pem                  # Certificates
credentials.json       # Credential files
```

### Using .env Files

1. **Copy the template**:
   ```bash
   cp .env.example .env
   ```

2. **Add your credentials**:
   ```bash
   # Edit .env with your actual values
   nano .env
   ```

3. **Verify .env is ignored**:
   ```bash
   git status --ignored | grep .env
   # Should show: .env (in gitignore)
   ```

## API Keys Management

### Crates.io API Key

**Required for publishing crates to crates.io**

1. **Generate Token**:
   - Visit [crates.io/me](https://crates.io/me)
   - Click "New Token"
   - Name: "Ruvector Publishing"
   - Permissions: "publish-new" and "publish-update"
   - Copy the token immediately (shown only once)

2. **Store Securely**:
   ```bash
   # Add to .env (which is gitignored)
   echo "CRATES_API_KEY=your-actual-token-here" >> .env
   ```

3. **Use from .env**:
   ```bash
   # Publishing script automatically loads from .env
   ./scripts/publish-crates.sh
   ```

### Key Rotation

Rotate API keys regularly:

```bash
# 1. Generate new token on crates.io
# 2. Update .env with new token
# 3. Test with: cargo login $CRATES_API_KEY
# 4. Revoke old token on crates.io
```

## Development Secrets

### What NOT to Commit

❌ **Never commit**:
- API keys (crates.io, npm, etc.)
- Database credentials
- Private keys (.key, .pem files)
- OAuth tokens
- Session secrets
- Encryption keys
- Service account credentials

✅ **Safe to commit**:
- `.env.example` (template with no real values)
- Public configuration
- Example data (non-sensitive)
- Documentation

### Pre-commit Checks

Before committing, verify no secrets are staged:

```bash
# Check staged files
git diff --staged

# Search for potential secrets
git diff --staged | grep -i "api_key\|secret\|password\|token"

# Use git-secrets (optional)
git secrets --scan
```

### GitHub Secret Scanning

GitHub automatically scans for common secrets. If detected:

1. **Immediately revoke** the exposed credential
2. **Generate a new** credential
3. **Update .env** with new credential
4. **Force push** to remove from history (if needed):
   ```bash
   # Dangerous! Only if absolutely necessary
   git filter-branch --force --index-filter \
     "git rm --cached --ignore-unmatch .env" \
     --prune-empty --tag-name-filter cat -- --all
   ```

## CI/CD Secrets

### GitHub Actions

Store secrets in GitHub repository settings:

1. Go to repository Settings → Secrets and variables → Actions
2. Add secrets:
   - `CRATES_API_KEY` - for publishing
   - `CODECOV_TOKEN` - for code coverage (optional)

3. Use in workflows:
   ```yaml
   - name: Publish to crates.io
     env:
       CARGO_REGISTRY_TOKEN: ${{ secrets.CRATES_API_KEY }}
     run: cargo publish
   ```

### Local Development

For local development, use `.env`:

```bash
# .env (gitignored)
CRATES_API_KEY=cio-xxx...
RUST_LOG=debug
```

Load in scripts:
```bash
# Load from .env
export $(grep -v '^#' .env | xargs)
```

## Code Signing

### Signing Releases

For production releases:

```bash
# Generate GPG key (if not exists)
gpg --gen-key

# Sign git tags
git tag -s v0.1.0 -m "Release v0.1.0"

# Verify signature
git tag -v v0.1.0
```

### Cargo Package Signing

Cargo doesn't support package signing yet, but you can:

1. Sign the git tag
2. Include checksums in release notes
3. Provide GPG signatures for binary releases

## Dependency Security

### Audit Dependencies

Regularly audit dependencies for vulnerabilities:

```bash
# Install cargo-audit
cargo install cargo-audit

# Run security audit
cargo audit

# Fix vulnerabilities
cargo audit fix
```

### Automated Scanning

Enable GitHub Dependabot:

1. Go to repository Settings → Security → Dependabot
2. Enable "Dependabot alerts"
3. Enable "Dependabot security updates"

## Reporting Security Issues

### Responsible Disclosure

If you discover a security vulnerability:

1. **Do NOT** open a public GitHub issue
2. **Email**: [security@ruv.io](mailto:security@ruv.io)
3. **Include**:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if any)

### Response Timeline

- **24 hours**: Initial response
- **7 days**: Status update
- **30 days**: Fix released (if confirmed)

## Security Checklist

Before releasing:

- [ ] No secrets in code or config files
- [ ] `.env` is in `.gitignore`
- [ ] `.env.example` has no real values
- [ ] All dependencies audited (`cargo audit`)
- [ ] Git tags are signed
- [ ] API keys rotated if exposed
- [ ] Security scan passed (GitHub)
- [ ] Documentation reviewed for sensitive info

## Resources

- [Cargo Security Guidelines](https://doc.rust-lang.org/cargo/reference/security.html)
- [GitHub Secret Scanning](https://docs.github.com/en/code-security/secret-scanning)
- [OWASP Top 10](https://owasp.org/www-project-top-ten/)
- [Rust Security Guidelines](https://anssi-fr.github.io/rust-guide/)

## Support

For security questions:
- Email: [security@ruv.io](mailto:security@ruv.io)
- Documentation: [docs.ruv.io](https://docs.ruv.io)
- Community: [Discord](https://discord.gg/ruvnet)
