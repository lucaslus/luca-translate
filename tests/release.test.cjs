const { test } = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const os = require("node:os");
const { prepare } = require("../scripts/prepare-release.cjs");
test("release aggregation requires all platforms and signatures, and disambiguates filenames", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "lucas-release-test-"));
  const input = path.join(root, "artifacts");
  fs.mkdirSync(input);
  const targets = [
    "darwin-aarch64",
    "darwin-x86_64",
    "windows-x86_64",
    "linux-x86_64",
  ];
  let last;
  for (const target of targets) {
    const dir = path.join(input, "signed-" + target);
    fs.mkdirSync(dir);
    const suffix = target.startsWith("darwin")
      ? ".app.tar.gz"
      : target.startsWith("windows")
        ? ".exe"
        : ".AppImage";
    last = path.join(dir, "lucas-translate" + suffix);
    fs.writeFileSync(last, "synthetic updater archive");
    fs.writeFileSync(last + ".sig", "c3ludGhldGlj");
  }
  const output = path.join(root, "release");
  // Arch is a required release asset, but never an updater archive.
  assert.throws(() => prepare(input, output));
  assert(!fs.existsSync(output));
  const archDir = path.join(input, "arch-x86_64");
  fs.mkdirSync(archDir);
  const version = require("../app/src-tauri/tauri.conf.json").version;
  const archName = `lucas-translate-${version}-1-x86_64.pkg.tar.zst`;
  const archPackage = path.join(archDir, archName);
  fs.writeFileSync(archPackage, "synthetic Arch package");
  const wrongVersion = path.join(
    archDir,
    "lucas-translate-999.0.0-1-x86_64.pkg.tar.zst",
  );
  fs.renameSync(archPackage, wrongVersion);
  assert.throws(() => prepare(input, output), /matching Arch Linux package/);
  assert(!fs.existsSync(output));
  fs.renameSync(wrongVersion, archPackage);
  fs.writeFileSync(wrongVersion, "duplicate");
  assert.throws(() => prepare(input, output), /matching Arch Linux package/);
  fs.unlinkSync(wrongVersion);
  const platforms = prepare(input, output);
  assert.equal(
    fs.readFileSync(path.join(output, archName), "utf8"),
    "synthetic Arch package",
  );
  const sums = fs.readFileSync(path.join(output, "SHA256SUMS"), "utf8");
  const hash = require("node:crypto")
    .createHash("sha256")
    .update("synthetic Arch package")
    .digest("hex");
  assert(sums.includes(`${hash}  ${archName}\n`));
  assert.deepEqual(Object.keys(platforms), targets);
  assert.notEqual(
    platforms["darwin-aarch64"].url,
    platforms["darwin-x86_64"].url,
  );
  assert.equal(
    fs.readFileSync(path.join(output, "SHA256SUMS"), "utf8").trim().split("\n")
      .length,
    10,
  );
  assert.throws(() => prepare(input, output));
  fs.renameSync(last + ".sig", last + ".missing");
  assert.throws(() => prepare(input, path.join(root, "incomplete")));
  assert(!fs.existsSync(path.join(root, "incomplete")));
});
