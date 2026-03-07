#!/usr/bin/env node

const { spawnSync } = require("child_process");
const path = require("path");
const fs = require("fs");

const PLATFORMS = {
  "darwin-arm64": "pick-cli-darwin-arm64",
  "darwin-x64": "pick-cli-darwin-x64",
  "linux-x64": "pick-cli-linux-x64",
  "linux-arm64": "pick-cli-linux-arm64",
  "win32-x64": "pick-cli-win32-x64",
};

const platformKey = `${process.platform}-${process.arch}`;
const pkgName = PLATFORMS[platformKey];

if (!pkgName) {
  console.error(
    `pick: unsupported platform ${process.platform}-${process.arch}`
  );
  console.error(`Supported: ${Object.keys(PLATFORMS).join(", ")}`);
  process.exit(1);
}

const binName = `pick${process.platform === "win32" ? ".exe" : ""}`;

function findBinary() {
  // 1. Try require.resolve (works when properly installed as dependency)
  try {
    return require.resolve(`${pkgName}/bin/${binName}`);
  } catch {}

  // 2. Walk up from the *executed* script path (process.argv[1]) to find
  //    node_modules. This handles symlinks from node_modules/.bin/ correctly
  //    since process.argv[1] gives the symlink target inside node_modules.
  const scriptPaths = [
    process.argv[1],   // The actual executed path (follows .bin symlink)
    __filename,        // May be resolved differently
  ];

  for (const scriptPath of scriptPaths) {
    if (!scriptPath) continue;
    let dir = path.dirname(scriptPath);
    while (dir !== path.dirname(dir)) {
      const candidate = path.join(
        dir, "node_modules", pkgName, "bin", binName
      );
      if (fs.existsSync(candidate)) {
        return candidate;
      }
      // Also check if we're inside node_modules/pick-cli/bin already
      // and the sibling package is at the same node_modules level
      if (dir.endsWith(path.join("node_modules", "pick-cli"))) {
        const sibling = path.join(
          path.dirname(dir), pkgName, "bin", binName
        );
        if (fs.existsSync(sibling)) {
          return sibling;
        }
      }
      dir = path.dirname(dir);
    }
  }

  return null;
}

const binPath = findBinary();

if (!binPath) {
  console.error(`pick: could not find binary package "${pkgName}".`);
  console.error(
    `This usually means the optional dependency was not installed.`
  );
  console.error(`Try: npm install ${pkgName}`);
  process.exit(1);
}

const result = spawnSync(binPath, process.argv.slice(2), {
  stdio: "inherit",
});

if (result.error) {
  console.error(`pick: ${result.error.message}`);
  process.exit(1);
}

process.exit(result.status ?? 1);
