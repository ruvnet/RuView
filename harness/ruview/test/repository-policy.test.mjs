import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
const provenanceFiles = [
  "harness/ruview/package.json",
  "harness/ruview/.harness/manifest.json",
  "harness/ruview/.harness/manifest.sha256",
];

function git(args, cwd = repoRoot) {
  return execFileSync("git", args, { cwd, encoding: "utf8" });
}

test("sensing-server session secret is ignored at every runtime path", () => {
  for (const path of ["data/session-secret", "v2/data/session-secret"]) {
    assert.doesNotThrow(() =>
      git(["check-ignore", "--no-index", "--quiet", "--", path]),
    );
  }
});

test("harness provenance files are checked out with LF line endings", () => {
  const attributes = git(["check-attr", "text", "eol", "--", ...provenanceFiles]);

  for (const file of provenanceFiles) {
    assert.match(attributes, new RegExp(`${file}: text: auto`));
    assert.match(attributes, new RegExp(`${file}: eol: lf`));
  }
});

test("core.autocrlf checkout preserves LF bytes for harness provenance", () => {
  const scratch = mkdtempSync(join(tmpdir(), "ruview-lf-checkout-"));
  const source = join(scratch, "source");
  const checkout = join(scratch, "checkout");

  try {
    mkdirSync(join(source, "harness/ruview/.harness"), { recursive: true });
    copyFileSync(join(repoRoot, ".gitattributes"), join(source, ".gitattributes"));
    writeFileSync(join(source, "harness/ruview/.harness/manifest.json"), '{\n  "ok": true\n}\n');
    writeFileSync(join(source, "harness/ruview/.harness/manifest.sha256"), "digest  manifest.json\n");

    git(["init", "--quiet"], source);
    git(["config", "user.email", "test@example.invalid"], source);
    git(["config", "user.name", "RuView test"], source);
    git(["add", "."], source);
    git(["commit", "--quiet", "-m", "test fixture"], source);
    execFileSync("git", ["-c", "core.autocrlf=true", "clone", "--quiet", source, checkout]);

    for (const relativePath of provenanceFiles.slice(1)) {
      const bytes = readFileSync(join(checkout, relativePath));
      assert.equal(bytes.includes(Buffer.from("\r\n")), false, `${relativePath} must remain LF`);
    }
  } finally {
    rmSync(scratch, { recursive: true, force: true });
  }
});
