/* eslint-disable */
/* auto-generated: NAPI-RS platform loader for @ruvector/rvf-node */

const { existsSync, readFileSync } = require('fs');
const { join } = require('path');

const { platform, arch } = process;

let nativeBinding = null;
let localFileExisted = false;
let loadError = null;

function isMusl() {
  // For Node 12+, check report.header.glibcVersionRuntime
  if (!process.report || typeof process.report.getReport !== 'function') {
    try {
      const lddPath = require('child_process')
        .execSync('which ldd')
        .toString()
        .trim();
      return readFileSync(lddPath, 'utf8').includes('musl');
    } catch {
      return true;
    }
  } else {
    const report = process.report.getReport();
    const rep = typeof report === 'string' ? JSON.parse(report) : report;
    return !rep.header.glibcVersionRuntime;
  }
}

switch (platform) {
  case 'darwin':
    switch (arch) {
      case 'x64':
        localFileExisted = existsSync(join(__dirname, 'rvf-node.darwin-x64.node'));
        try {
          if (localFileExisted) {
            nativeBinding = require('./rvf-node.darwin-x64.node');
          } else {
            nativeBinding = require('@ruvector/rvf-node-darwin-x64');
          }
        } catch (e) {
          loadError = e;
        }
        break;
      case 'arm64':
        localFileExisted = existsSync(join(__dirname, 'rvf-node.darwin-arm64.node'));
        try {
          if (localFileExisted) {
            nativeBinding = require('./rvf-node.darwin-arm64.node');
          } else {
            nativeBinding = require('@ruvector/rvf-node-darwin-arm64');
          }
        } catch (e) {
          loadError = e;
        }
        break;
      default:
        throw new Error(`Unsupported architecture on macOS: ${arch}`);
    }
    break;
  case 'win32':
    switch (arch) {
      case 'x64':
        localFileExisted = existsSync(join(__dirname, 'rvf-node.win32-x64-msvc.node'));
        try {
          if (localFileExisted) {
            nativeBinding = require('./rvf-node.win32-x64-msvc.node');
          } else {
            nativeBinding = require('@ruvector/rvf-node-win32-x64-msvc');
          }
        } catch (e) {
          loadError = e;
        }
        break;
      default:
        throw new Error(`Unsupported architecture on Windows: ${arch}`);
    }
    break;
  case 'linux':
    switch (arch) {
      case 'x64':
        if (isMusl()) {
          localFileExisted = existsSync(join(__dirname, 'rvf-node.linux-x64-musl.node'));
          try {
            if (localFileExisted) {
              nativeBinding = require('./rvf-node.linux-x64-musl.node');
            } else {
              nativeBinding = require('@ruvector/rvf-node-linux-x64-musl');
            }
          } catch (e) {
            loadError = e;
          }
        } else {
          localFileExisted = existsSync(join(__dirname, 'rvf-node.linux-x64-gnu.node'));
          try {
            if (localFileExisted) {
              nativeBinding = require('./rvf-node.linux-x64-gnu.node');
            } else {
              nativeBinding = require('@ruvector/rvf-node-linux-x64-gnu');
            }
          } catch (e) {
            loadError = e;
          }
        }
        break;
      case 'arm64':
        if (isMusl()) {
          localFileExisted = existsSync(join(__dirname, 'rvf-node.linux-arm64-musl.node'));
          try {
            if (localFileExisted) {
              nativeBinding = require('./rvf-node.linux-arm64-musl.node');
            } else {
              nativeBinding = require('@ruvector/rvf-node-linux-arm64-musl');
            }
          } catch (e) {
            loadError = e;
          }
        } else {
          localFileExisted = existsSync(join(__dirname, 'rvf-node.linux-arm64-gnu.node'));
          try {
            if (localFileExisted) {
              nativeBinding = require('./rvf-node.linux-arm64-gnu.node');
            } else {
              nativeBinding = require('@ruvector/rvf-node-linux-arm64-gnu');
            }
          } catch (e) {
            loadError = e;
          }
        }
        break;
      default:
        throw new Error(`Unsupported architecture on Linux: ${arch}`);
    }
    break;
  default:
    throw new Error(`Unsupported OS: ${platform}, architecture: ${arch}`);
}

if (!nativeBinding) {
  if (loadError) {
    throw loadError;
  }
  throw new Error('Failed to load native binding');
}

const { RvfDatabase } = nativeBinding;

module.exports.RvfDatabase = RvfDatabase;
