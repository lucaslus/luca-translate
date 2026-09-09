// Aggregate only after all platforms succeeded; never publish a partial update manifest.
const fs = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");
const root = path.resolve(__dirname, "..");
function version() {
  const v = require("../app/src-tauri/tauri.conf.json").version;
  const rust = fs
    .readFileSync(path.join(root, "app/src-tauri/Cargo.toml"), "utf8")
    .match(/^version = "([^"]+)"/m)?.[1];
  if (!/^\d+\.\d+\.\d+$/.test(v) || rust !== v)
    throw Error("Use the same stable semver in Cargo.toml and tauri.conf.json");
  return v;
}
function walk(dir) {
  return fs
    .readdirSync(dir, { withFileTypes: true })
    .flatMap((e) =>
      e.isDirectory()
        ? walk(path.join(dir, e.name))
        : e.isFile()
          ? [path.join(dir, e.name)]
          : [],
    );
}
function prepare(input, output) {
  const v = version();
  const platforms = {};
  const files = [];
  for (const target of [
    "darwin-aarch64",
    "darwin-x86_64",
    "windows-x86_64",
    "linux-x86_64",
  ]) {
    const assets = walk(path.join(input, "signed-" + target)).filter((p) =>
      /\.(dmg|app\.tar\.gz|sig|exe|AppImage|deb|rpm)$/.test(p),
    );
    const suffix = target.startsWith("darwin")
      ? ".app.tar.gz"
      : target.startsWith("windows")
        ? ".exe"
        : ".AppImage";
    const installers = assets.filter((p) => p.endsWith(suffix));
    if (installers.length !== 1)
      throw Error("Expected one updater archive for " + target);
    const installer = installers[0];
    const signature = fs.readFileSync(installer + ".sig", "utf8").trim();
    if (!signature || !/^[A-Za-z0-9+/=]+$/.test(signature))
      throw Error("Missing/invalid signature: " + target);
    const name = target + "-" + path.basename(installer);
    platforms[target] = {
      signature,
      url: `https://github.com/lucaslus/luca-translate/releases/download/v${v}/${encodeURIComponent(name)}`,
    };
    for (const file of assets)
      files.push({ file, name: target + "-" + path.basename(file) });
  }
  const archName = `lucas-translate-${v}-1-x86_64.pkg.tar.zst`;
  const archPackages = walk(path.join(input, "arch-x86_64")).filter((p) =>
    p.endsWith(".pkg.tar.zst"),
  );
  if (archPackages.length !== 1 || path.basename(archPackages[0]) !== archName)
    throw Error("Expected one matching Arch Linux package: " + archName);
  files.push({ file: archPackages[0], name: archName });
  if (new Set(files.map((f) => f.name)).size !== files.length)
    throw Error("Duplicate release asset names");
  // Exclusive creation prevents accidentally overwriting an already prepared release.
  fs.mkdirSync(output, { recursive: false });
  for (const { file, name } of files)
    fs.copyFileSync(file, path.join(output, name), fs.constants.COPYFILE_EXCL);
  fs.writeFileSync(
    path.join(output, "latest.json"),
    JSON.stringify(
      {
        version: v,
        notes: `Lucas Translate ${v}`,
        pub_date: new Date().toISOString(),
        platforms,
      },
      null,
      2,
    ),
    { flag: "wx" },
  );
  const checksums = fs
    .readdirSync(output)
    .sort()
    .map(
      (name) =>
        crypto
          .createHash("sha256")
          .update(fs.readFileSync(path.join(output, name)))
          .digest("hex") +
        "  " +
        name,
    );
  fs.writeFileSync(
    path.join(output, "SHA256SUMS"),
    checksums.join("\n") + "\n",
    { flag: "wx" },
  );
  return platforms;
}
if (require.main === module) {
  if (process.argv[2] === "--check-version")
    console.log("Version verified: " + version());
  else {
    const input = path.resolve(process.argv[2] || "artifacts");
    const output = path.resolve(process.argv[3] || "dist/release");
    fs.mkdirSync(path.dirname(output), { recursive: true });
    prepare(input, output);
    console.log(
      "Prepared four-platform signed manifest. No release was published.",
    );
  }
}
module.exports = { prepare };
