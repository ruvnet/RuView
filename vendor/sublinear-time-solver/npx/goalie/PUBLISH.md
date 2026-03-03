# Publishing Goalie to npm

## 📦 Package Ready for Publishing

The Goalie package is now properly configured for npm publishing with:

- ✅ Proper package.json with all required fields
- ✅ MIT License file
- ✅ .npmignore to exclude dev files
- ✅ CLI with shebang for npx execution
- ✅ Pre-publish build scripts
- ✅ TypeScript compilation

## 🚀 Publishing Steps

### 1. Prerequisites

```bash
# Ensure you're logged into npm
npm login

# Verify your npm account
npm whoami
```

### 2. Pre-publish Check

```bash
# Clean and build
npm run clean
npm run build

# Test the package locally
npm pack
# This creates a .tgz file - inspect it to ensure only necessary files are included

# Test CLI works
node dist/cli.js --help
```

### 3. Version Management

```bash
# For patch version (1.0.0 -> 1.0.1)
npm version patch

# For minor version (1.0.0 -> 1.1.0)
npm version minor

# For major version (1.0.0 -> 2.0.0)
npm version major
```

### 4. Publish to npm

```bash
# Dry run to see what will be published
npm publish --dry-run

# Actual publish
npm publish

# For first time publishing with specific tag
npm publish --tag latest
```

## 🔍 Verification After Publishing

```bash
# Test npx command works
npx goalie --help

# Test MCP server starts
npx goalie start

# Check package on npm
npm view goalie
```

## 📋 Package Contents

The published package includes:
- `dist/` - Compiled JavaScript
- `README.md` - Documentation
- `LICENSE` - MIT license
- `package.json` - Package configuration

## 🔄 Updating the Package

For future updates:

```bash
# 1. Make your changes
# 2. Update version
npm version patch  # or minor/major

# 3. Publish
npm publish
```

## 📝 Notes

- The package name "goalie" must be available on npm
- If name is taken, consider:
  - `@ruv/goalie`
  - `goalie-ai`
  - `goalie-research`
- Remember to update GitHub repository URL in package.json if different

## 🎯 Post-Publish

After successful publishing:

1. **Update README badges** with npm version
2. **Create GitHub release** matching npm version
3. **Test installation** on clean system:
   ```bash
   npx goalie --help
   ```
4. **Share** the good news! 🎉