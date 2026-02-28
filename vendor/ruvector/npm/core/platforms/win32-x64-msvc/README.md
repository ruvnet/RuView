# ruvector-core-win32-x64-msvc

[![npm version](https://badge.fury.io/js/ruvector-core-win32-x64-msvc.svg)](https://www.npmjs.com/package/ruvector-core-win32-x64-msvc)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

**Windows x64 MSVC native binding for ruvector-core**

This package contains the native Node.js binding (`.node` file) for Windows x64 systems compiled with MSVC. It is automatically installed as an optional dependency when you install `ruvector-core` on a compatible system.

🌐 **[Visit ruv.io](https://ruv.io)** for more AI infrastructure tools

## Installation

**You should not install this package directly.** Instead, install the main package:

```bash
npm install ruvector-core
```

The correct platform-specific package will be automatically installed based on your system.

## System Requirements

- **Operating System**: Windows 10 (1809+) or Windows 11, Windows Server 2019+
- **Architecture**: x86_64 (64-bit)
- **Node.js**: 18.0.0 or higher
- **Visual C++ Runtime**: Automatically included with Node.js

## Compatibility

This package is compatible with:
- **Windows 10** (version 1809 or later)
- **Windows 11** (all versions)
- **Windows Server 2019** and newer
- Most Windows development environments

**Note:** Windows ARM64 is not currently supported.

## What's Inside

This package contains:
- **ruvector.node** - Native binary module compiled from Rust with MSVC
- **index.js** - Module loader with error handling
- Full HNSW indexing implementation
- SIMD-optimized vector operations (AVX2, SSE4.2)
- Multi-threaded async operations via Tokio

## Performance

When running on Windows x64 systems, you can expect:
- **50,000+ vector inserts per second**
- **10,000+ searches per second** (k=10)
- **~50 bytes memory per 128-dim vector**
- **Sub-millisecond latency** for most operations
- Optimized for Intel/AMD AVX2 SIMD instructions

## Building from Source

If you need to rebuild the native module:

### Prerequisites

1. Install **Visual Studio 2022** (or 2019) with "Desktop development with C++" workload
2. Install **Rust**: https://rustup.rs/
3. Open "x64 Native Tools Command Prompt for VS 2022"

### Build Steps

```bash
# Clone the repository
git clone https://github.com/ruvnet/ruvector.git
cd ruvector

# Build for Windows x64
cd npm\packages\core
npm run build:napi -- --target x86_64-pc-windows-msvc
```

## Troubleshooting

### Module Not Found Error

If you see "Cannot find module 'ruvector-core-win32-x64-msvc'":

1. Verify you're on Windows 64-bit: `wmic os get osarchitecture` should show "64-bit"
2. Reinstall with optional dependencies: `npm install --include=optional ruvector-core`
3. Check Node.js version: `node --version` should be 18.0.0 or higher

### DLL Loading Issues

If the module fails to load with DLL errors:

1. **Install Visual C++ Redistributable**:
   - Download from: https://aka.ms/vs/17/release/vc_redist.x64.exe
   - Node.js usually includes this, but manual install may be needed

2. **Check Windows Updates**:
   - Ensure Windows is up to date
   - Some MSVC runtimes come through Windows Update

3. **Verify Node.js Installation**:
   - Reinstall Node.js from nodejs.org
   - Use the Windows Installer (.msi) version

### Long Path Issues

If you encounter "path too long" errors:

1. **Enable Long Paths in Windows**:
   ```powershell
   # Run PowerShell as Administrator
   New-ItemProperty -Path "HKLM:\SYSTEM\CurrentControlSet\Control\FileSystem" -Name "LongPathsEnabled" -Value 1 -PropertyType DWORD -Force
   ```

2. **Or use shorter paths**:
   - Install Node modules closer to drive root (e.g., `C:\projects\`)

### Antivirus False Positives

Some antivirus software may flag native `.node` files:
- Add an exception for `node_modules\ruvector-core-win32-x64-msvc\`
- Or temporarily disable real-time scanning during npm install

### WSL2 (Windows Subsystem for Linux)

If you're using WSL2:
- Use the Linux packages instead (`ruvector-core-linux-x64-gnu`)
- This Windows package is for native Windows Node.js only

## Related Packages

- **[ruvector-core](https://www.npmjs.com/package/ruvector-core)** - Main package (install this)
- **[ruvector-core-linux-x64-gnu](https://www.npmjs.com/package/ruvector-core-linux-x64-gnu)** - Linux x64
- **[ruvector-core-linux-arm64-gnu](https://www.npmjs.com/package/ruvector-core-linux-arm64-gnu)** - Linux ARM64
- **[ruvector-core-darwin-x64](https://www.npmjs.com/package/ruvector-core-darwin-x64)** - macOS Intel
- **[ruvector-core-darwin-arm64](https://www.npmjs.com/package/ruvector-core-darwin-arm64)** - macOS Apple Silicon

## Resources

- 🏠 [Homepage](https://ruv.io)
- 📦 [GitHub Repository](https://github.com/ruvnet/ruvector)
- 📚 [Documentation](https://github.com/ruvnet/ruvector/tree/main/docs)
- 🐛 [Issue Tracker](https://github.com/ruvnet/ruvector/issues)

## License

MIT License - see [LICENSE](https://github.com/ruvnet/ruvector/blob/main/LICENSE) for details.

---

Built with ❤️ by the [ruv.io](https://ruv.io) team
