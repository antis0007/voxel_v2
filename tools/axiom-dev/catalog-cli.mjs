import { catalogContracts, describeCatalog } from "./catalog-contracts.mjs";

const [command = "list", target, ...extra] = process.argv.slice(2);
try {
  if (extra.length || !["list", "describe"].includes(command) || (command === "list" && target) || (command === "describe" && !target)) {
    throw new Error("Usage: catalog-cli.mjs list | describe <id-or-path>");
  }
  console.log(JSON.stringify(command === "list" ? { scope: "Native-validated authoring catalogs only", catalogs: catalogContracts }
    : describeCatalog(target), null, 2));
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
