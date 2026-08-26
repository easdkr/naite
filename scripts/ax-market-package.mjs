#!/usr/bin/env node
import { createHash } from "node:crypto";
import { createReadStream, statSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const args = parseArgs(process.argv.slice(2));

if (args.help) {
  printHelp();
  process.exit(0);
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : error);
  process.exit(1);
});

async function main() {
  ensureMacosTool("codesign");
  ensureMacosTool("ditto");

  const cargoVersion = commandOutput("cargo", ["pkgid", "-p", "naite-app"])
    .split("#")
    .pop()
    ?.trim();
  if (!cargoVersion) {
    throw new Error("Could not resolve naite-app package version.");
  }

  const shortSha = commandOutput("git", ["rev-parse", "--short", "HEAD"]).trim();
  const buildStamp = new Date()
    .toISOString()
    .replace(/\D/g, "")
    .slice(0, 14);
  const artifactVersion =
    option("version", "NAITE_AX_MARKET_VERSION") ??
    `${cargoVersion}+adhoc.${buildStamp}.${shortSha}`;
  const bundleVersion =
    option("bundle-version", "NAITE_BUNDLE_VERSION") ?? buildStamp;

  const bundlePath = buildBundle({
    bundleShortVersion: cargoVersion,
    bundleVersion,
  });
  adHocSign(bundlePath);

  const host = commandOutput("rustc", ["-vV"]);
  const arch = axArchitecture(host);
  const zipPath = packageZip(bundlePath, artifactVersion, arch);
  const metadata = await fileMetadata(zipPath);

  console.log(`Built AX market artifact: ${zipPath}`);
  console.log(`SHA-256: ${metadata.sha256}`);
  console.log(`Size: ${metadata.byteSize} bytes`);
  console.log(`Platform: MACOS`);
  console.log(`Architecture: ${arch}`);
  console.log(`Content-Type: application/zip`);
  console.log("Upload this zip manually in AX app market.");
}

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (!arg.startsWith("--")) {
      throw new Error(`Unexpected argument: ${arg}`);
    }

    const raw = arg.slice(2);
    const equalsIndex = raw.indexOf("=");
    const rawKey = equalsIndex === -1 ? raw : raw.slice(0, equalsIndex);
    const inlineValue =
      equalsIndex === -1 ? undefined : raw.slice(equalsIndex + 1);

    if (rawKey === "help") {
      parsed.help = true;
      continue;
    }

    const value = inlineValue ?? argv[index + 1];
    if (!value || value.startsWith("--")) {
      throw new Error(`Missing value for --${rawKey}.`);
    }

    parsed[rawKey] = value;
    if (inlineValue === undefined) {
      index += 1;
    }
  }
  return parsed;
}

function option(argName, envName) {
  const value = args[argName] ?? process.env[envName];
  if (typeof value !== "string") return undefined;
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : undefined;
}

function buildBundle({ bundleShortVersion, bundleVersion }) {
  const result = spawnSync("scripts/macos-bundle.sh", ["--release"], {
    cwd: repoRoot,
    env: {
      ...process.env,
      PATH: `${process.env.HOME}/.cargo/bin:${process.env.PATH ?? ""}`,
      NAITE_BUNDLE_SHORT_VERSION: bundleShortVersion,
      NAITE_BUNDLE_VERSION: bundleVersion,
    },
    encoding: "utf8",
  });

  if (result.status !== 0) {
    throw commandError("scripts/macos-bundle.sh", result);
  }

  const bundlePath = result.stdout.trim().split(/\r?\n/).pop();
  if (!bundlePath) {
    throw new Error("macOS bundle script did not print a bundle path.");
  }
  return bundlePath;
}

function adHocSign(bundlePath) {
  run("codesign", ["--force", "--deep", "--sign", "-", bundlePath]);
  run("codesign", ["--verify", "--deep", "--strict", "--verbose=2", bundlePath]);
}

function packageZip(bundlePath, artifactVersion, arch) {
  const targetDir = dirname(bundlePath);
  const safeVersion = artifactVersion.replace(/[^A-Za-z0-9._-]/g, "_");
  const zipPath = join(
    targetDir,
    `naite-${safeVersion}-macos-${arch.toLowerCase()}.zip`,
  );
  run("rm", ["-f", zipPath]);
  run("ditto", ["-c", "-k", "--keepParent", bundlePath, zipPath]);
  return zipPath;
}

async function fileMetadata(path) {
  const hash = createHash("sha256");
  await new Promise((resolve, reject) => {
    createReadStream(path)
      .on("data", (chunk) => hash.update(chunk))
      .on("error", reject)
      .on("end", resolve);
  });

  return {
    byteSize: statSync(path).size,
    sha256: hash.digest("hex"),
  };
}

function axArchitecture(rustcVersionOutput) {
  const host = rustcVersionOutput
    .split(/\r?\n/)
    .find((line) => line.startsWith("host: "));
  if (!host) {
    throw new Error("Could not resolve Rust host architecture.");
  }
  if (host.includes("aarch64-apple-darwin")) return "ARM64";
  if (host.includes("x86_64-apple-darwin")) return "X64";
  throw new Error(`Unsupported AX market architecture for ${host}.`);
}

function ensureMacosTool(name) {
  const result = spawnSync("sh", ["-c", `command -v ${name}`], {
    encoding: "utf8",
  });
  if (result.status !== 0) {
    throw new Error(`${name} is required to build the AX market macOS zip.`);
  }
}

function commandOutput(command, commandArgs) {
  const result = spawnSync(command, commandArgs, {
    cwd: repoRoot,
    env: {
      ...process.env,
      PATH: `${process.env.HOME}/.cargo/bin:${process.env.PATH ?? ""}`,
    },
    encoding: "utf8",
  });
  if (result.status !== 0) {
    throw commandError(command, result);
  }
  return result.stdout;
}

function run(command, commandArgs) {
  const result = spawnSync(command, commandArgs, {
    cwd: repoRoot,
    env: process.env,
    stdio: "inherit",
  });
  if (result.status !== 0) {
    throw commandError(command, result);
  }
}

function commandError(command, result) {
  return new Error(
    [
      `${command} failed with exit code ${result.status}.`,
      result.stderr?.trim(),
      result.stdout?.trim(),
    ]
      .filter(Boolean)
      .join("\n"),
  );
}

function printHelp() {
  console.log(`
Usage:
  npm run build:ax-market -- [options]

Builds target/release/naite.app, ad-hoc signs it, and packages a zip for manual
upload to AX app market.

Options:
  --version <semver>         Artifact version (default: 0.1.0+adhoc.timestamp.sha)
  --bundle-version <value>   CFBundleVersion override (default: timestamp)
`);
}
