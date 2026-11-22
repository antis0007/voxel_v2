// Conservative lexical protection for automatic Rust repairs. This is not a Rust parser
// or evidence that tests ran. Native candidates still require the independent native gate.
function visibleRust(text) {
  const out = text.split("");
  const hide = (start, end) => { for (let i = start; i < end; i++) if (out[i] !== "\n" && out[i] !== "\r") out[i] = " "; };
  for (let i = 0; i < text.length;) {
    const start = i;
    if (text.startsWith("//", i)) {
      i = text.indexOf("\n", i); if (i < 0) i = text.length;
    } else if (text.startsWith("/*", i)) {
      i += 2; let depth = 1;
      while (i < text.length && depth) {
        if (text.startsWith("/*", i)) { depth++; i += 2; }
        else if (text.startsWith("*/", i)) { depth--; i += 2; }
        else i++;
      }
      if (depth) throw new Error("Unclosed comment; tests cannot be checked safely");
    } else {
      const raw = /^(?:br|cr|r)(#{0,32})"/.exec(text.slice(i));
      if (raw && (i === 0 || !/\w/.test(text[i - 1]))) {
        const end = text.indexOf('"' + raw[1], i + raw[0].length);
        if (end < 0) throw new Error("Unclosed raw string; tests cannot be checked safely");
        i = end + 1 + raw[1].length;
      } else if (text[i] === '"') {
        i++; let closed = false;
        while (i < text.length) {
          if (text[i] === "\\") i += 2;
          else if (text[i++] === '"') { closed = true; break; }
        }
        if (!closed) throw new Error("Unclosed string; tests cannot be checked safely");
      } else if (text[i] === "'") {
        // A lifetime is not a character literal.
        const character = /^'(?:[^'\\\r\n]|\\(?:u\{[a-fA-F0-9_]+\}|x[a-fA-F0-9]{2}|.))'/.exec(text.slice(i));
        if (character) i += character[0].length;
        else { i++; continue; }
      } else { i++; continue; }
    }
    hide(start, i);
  }
  return out.join("");
}

function testBoundaries(code) {
  const marker = /^[ \t]*#\[\s*(cfg\(\s*test\s*\)|test)\s*\]|^[ \t]*mod\s+tests\s*\{/gm;
  const declarations = [];
  for (const match of code.matchAll(marker)) {
    const end = match.index + match[0].length;
    // Preserve leading test-only imports and external module declarations in
    // their original scope. Production items can follow those declarations.
    const declaration = match[1]?.startsWith("cfg(")
      && /^\s*(?:#\[[^\]\r\n]*\]\s*)*(?:use\b[^;]*|mod\s+[A-Za-z_]\w*\s*);/.exec(code.slice(end));
    if (!declaration) return { declarations, suffixStart: match.index };
    declarations.push({ start: match.index, end: end + declaration[0].length });
  }
  return { declarations, suffixStart: undefined };
}

export function assertFrozenRustTests(before, after) {
  const oldCode = visibleRust(before), newCode = visibleRust(after);
  const oldTest = testBoundaries(oldCode), newTest = testBoundaries(newCode);
  const fail = () => { throw new Error("Automatic improvement cannot rewrite, remove, enclose or disable existing tests"); };
  const oldDeclarations = oldTest.declarations, newDeclarations = newTest.declarations;
  if (oldDeclarations.length !== newDeclarations.length) fail();
  if (before.slice(0, oldDeclarations[0]?.end ?? 0) !== after.slice(0, newDeclarations[0]?.end ?? 0)) fail();
  if ((oldTest.suffixStart === undefined) !== (newTest.suffixStart === undefined)) fail();
  if (oldTest.suffixStart === undefined && !oldDeclarations.length) return;
  if (oldTest.suffixStart !== undefined && before.slice(oldTest.suffixStart) !== after.slice(newTest.suffixStart)) fail();
  // New cfg/ignore attributes, imported macro overrides and macro definitions can
  // disable unchanged tests. Require manual review for those edits anywhere in a file.
  const directives = (text, code) => [...code.matchAll(/#!?\[[\s\S]*?\]|\b(?:use|extern\s+crate)\s+[^;]+;|\bmacro_rules\s*!\s*\w+|\bcfg\s*!\s*\([^)]*\)/g)]
    .map(match => text.slice(match.index, match.index + match[0].length));
  if (JSON.stringify(directives(before, oldCode)) !== JSON.stringify(directives(after, newCode))) {
    throw new Error("Automatic improvement cannot change imports, attributes or macros in a file with existing tests. Use fully qualified names and preserve the original directives.");
  }
  const enclosure = code => {
    const stack = [];
    for (const char of code) {
      if ("{([".includes(char)) stack.push(char);
      else if ("})]".includes(char) && stack.pop() !== ({ "}": "{", ")": "(", "]": "[" })[char]) fail();
    }
    return stack.join("");
  };
  for (const [index, original] of oldDeclarations.entries()) {
    const candidate = newDeclarations[index];
    if (before.slice(original.start, original.end) !== after.slice(candidate.start, candidate.end) ||
        enclosure(oldCode.slice(0, original.start)) !== enclosure(newCode.slice(0, candidate.start))) fail();
  }
  if (oldTest.suffixStart !== undefined &&
      enclosure(oldCode.slice(0, oldTest.suffixStart)) !== enclosure(newCode.slice(0, newTest.suffixStart))) fail();
}
