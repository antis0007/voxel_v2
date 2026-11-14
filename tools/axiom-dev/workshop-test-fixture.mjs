import { copyFileSync, mkdirSync, readdirSync } from "node:fs";
import path from "node:path";
import { nativeSourcePaths } from "./native-content-validation.mjs";

export function copyNativeFixture(source, destination) {
  const copy = relative => {
    mkdirSync(path.dirname(path.join(destination, relative)), { recursive: true });
    copyFileSync(path.join(source, relative), path.join(destination, relative));
  };
  for (const relative of nativeSourcePaths(source)) copy(relative);
  for (const name of readdirSync(path.join(source, "tools/axiom-dev"))) {
    if (name.endsWith(".mjs")) copy("tools/axiom-dev/" + name);
  }
  for (const name of readdirSync(path.join(source, "target/axiom-dev/native-validation"))) {
    copy("target/axiom-dev/native-validation/" + name);
  }
}
