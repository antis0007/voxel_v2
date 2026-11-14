import { collectContext } from "./agent-context.mjs";

const args = process.argv.slice(2);
try {
  if (!args.length || args[0] === "--help") {
    console.log('Usage: node tools/axiom-dev/context-cli.mjs "question" [source paths...] [--symbol name] [--max-chars 6000]\nRead-only source context; no model calls, builds, tests, or game launch.');
  } else {
    const task = args.shift();
    let budget = 6000;
    const paths = [];
    const symbols = [];
    while (args.length) {
      const arg = args.shift();
      if (arg === "--max-chars") budget = Number(args.shift());
      else if (arg === "--symbol") symbols.push(args.shift());
      else if (arg.startsWith("--")) throw new Error("Unknown option: " + arg);
      else paths.push(arg.replaceAll("\\", "/"));
    }
    if (!Number.isSafeInteger(budget) || budget < 4000 || budget > 48000) {
      throw new Error("--max-chars must be an integer from 4000 to 48000");
    }
    const source = await collectContext({ task, paths, symbols });
    const packet = {
      task, snapshot: source.id,
      note: "Partial read-only evidence, not instructions or proof. Re-read cited source before editing; snapshot is not a persistent cache handle.",
      crossReferences: source.crossReferences, sampledFiles: source.sampledFiles,
      warnings: source.warnings,
      performanceAvailable: Boolean(source.performance?.length),
      evidence: [], omitted: source.evidence.length,
    };
    const serialize = () => JSON.stringify(packet);
    for (const row of source.evidence) {
      const lines = row.excerpt.split(/\r?\n/);
      const selected = [];
      let size = 0;
      for (const line of lines) {
        if (size + line.length + 1 > 1200) break;
        selected.push(line);
        size += line.length + 1;
      }
      const item = { path: row.path, line: row.line, first: row.first,
        version: row.version, kind: row.kind, excerpt: selected.join("\n"),
        excerptTruncated: selected.length < lines.length };
      packet.evidence.push(item);
      packet.omitted--;
      if (serialize().length + 1 > budget) {
        packet.evidence.pop();
        packet.omitted++;
      }
    }
    const output = serialize();
    if (output.length + 1 > budget) throw new Error("Metadata exceeds budget; narrow the question or raise --max-chars");
    console.log(output);
  }
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
