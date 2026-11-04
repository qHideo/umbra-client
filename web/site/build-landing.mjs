#!/usr/bin/env node
// Stages the public Umbra landing page and its local brand assets for Pages.
import { cp, mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const args = process.argv.slice(2);
function option(name) {
  const index = args.indexOf(name);
  return index < 0 ? undefined : args[index + 1];
}
const outputArg = option("--output");
const repository = option("--repository");
if (!outputArg || !repository || !/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repository)) {
  process.stderr.write("Usage: node web/site/build-landing.mjs --output <directory> --repository <owner/repo>\n");
  process.exit(2);
}

const output = path.resolve(outputArg);
await mkdir(output, { recursive: true });
const releaseAssets = JSON.parse(await readFile(path.join(root, "packaging/release-assets.json"), "utf8"));
const platformLabels = { windows: "Windows", linux: "Linux", macos: "macOS", android: "Android" };
const downloadChoices = [];
for (const [kind, group] of [
  ["portable", releaseAssets.portable_assets],
  ["standard", releaseAssets.assets],
  ["manual", releaseAssets.manual_assets],
]) {
  for (const [platform, architectures] of Object.entries(group ?? {})) {
    for (const [architecture, filename] of Object.entries(architectures)) {
      const platformLabel = platformLabels[platform] ?? platform;
      const architectureLabel = platform === "macos"
        ? ({ arm64: "Apple silicon", x86_64: "Intel" }[architecture] ?? architecture)
        : platform === "windows"
          ? "x64"
          : platform === "android" && architecture.startsWith("arm64")
            ? "ARM64"
            : architecture;
      const suffix = kind === "portable" ? " · portable ZIP" : ` · ${architectureLabel}`;
      downloadChoices.push({ label: `${platformLabel}${suffix}`, filename });
    }
  }
}
if (downloadChoices.length === 0) throw new Error("No release download assets are configured");
let page = await readFile(path.join(root, "web/site/umbra.html"), "utf8");
page = page.replaceAll("../../assets/branding/", "assets/branding/")
  .replaceAll("__UMBRA_GITHUB_REPOSITORY__", repository)
  .replaceAll("__UMBRA_DOWNLOAD_CHOICES__", JSON.stringify(downloadChoices).replaceAll("<", "\\u003c"))
  .replaceAll("REPLACE_WITH_UMBRAS_GITHUB_OWNER/umbra-client", repository);
await writeFile(path.join(output, "index.html"), page);
const branding = path.join(output, "assets/branding");
await mkdir(branding, { recursive: true });
for (const filename of ["client-badge.png", "title.png"]) {
  await cp(path.join(root, "assets/branding", filename), path.join(branding, filename));
}
process.stdout.write(`Staged Umbra landing page for ${repository} in ${output}\n`);
