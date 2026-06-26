// One-shot helper to port an Android Compose screen file into :ui/commonMain:
// rewrites package, core-client access, IO dispatcher, strips Android-only
// imports, and converts 0-arg stringResource(R.string.NAME) -> tr("canonical.key")
// using the exact reverse of catalog.ts androidName(). Arg'd stringResource /
// painterResource references are left for manual handling.
// Usage: bun scripts/i18n/port-screen.ts <srcKt> <dstKt>
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const root = join(import.meta.dir, "..", "..");
const [src, dst] = process.argv.slice(2);

const androidName = (k: string): string =>
  k
    .replace(/[^A-Za-z0-9]+/g, "_")
    .replace(/([a-z0-9])([A-Z])/g, "$1_$2")
    .toLowerCase();

const en = JSON.parse(readFileSync(join(root, "locales/en.json"), "utf8")) as Record<string, string>;
const rev: Record<string, string> = {};
for (const k of Object.keys(en)) rev[androidName(k)] = k;

const stripImport =
  /^import (android\.|java\.|androidx\.core\.|androidx\.activity\.|org\.json\.|androidx\.compose\.ui\.platform\.LocalContext|androidx\.compose\.ui\.graphics\.asImageBitmap|androidx\.compose\.ui\.viewinterop\.AndroidView|androidx\.compose\.ui\.res\.stringResource|androidx\.compose\.ui\.res\.painterResource)/;

let c = readFileSync(src, "utf8");
c = c.replace(/^package jp\.nonbili\.meron$/m, "package jp.nonbili.meron.ui");
c = c.replace(/MeronCoreNative\.isLoaded\(\)/g, "coreLoaded");
c = c.replace(/MobileMailCommandClient\(JniMeronCore\(\)\)/g, "MobileMailCommandClient(core)");
c = c.replace(/Dispatchers\.IO/g, "ioDispatcher");
c = c
  .split("\n")
  .filter((l) => !stripImport.test(l))
  .join("\n");
c = c.replace(/stringResource\(R\.string\.([A-Za-z0-9_]+)\)/g, (m, name: string) => {
  const key = rev[name];
  if (!key) {
    console.error("NO CANONICAL KEY for R.string." + name);
    return m;
  }
  return `tr("${key}")`;
});

writeFileSync(dst, c);
console.log(`ported ${src} -> ${dst}`);
