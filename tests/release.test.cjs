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
  const platforms = prepare(input, output);
  assert.deepEqual(Object.keys(platforms), targets);
  assert.notEqual(
    platforms["darwin-aarch64"].url,
    platforms["darwin-x86_64"].url,
  );
  assert.equal(
    fs.readFileSync(path.join(output, "SHA256SUMS"), "utf8").trim().split("\n")
      .length,
    9,
  );
  assert.throws(() => prepare(input, output));
  fs.renameSync(last + ".sig", last + ".missing");
  assert.throws(() => prepare(input, path.join(root, "incomplete")));
  assert(!fs.existsSync(path.join(root, "incomplete")));
});
