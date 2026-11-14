// Conservative lexical protection for automatic Rust repairs. This is not a Rust parser
// or evidence that tests ran. Native candidates still require the independent native gate.
function visibleRust(text) {
  const out = [...text];
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

export function assertFrozenRustTests(before, after) {
  const oldCode = visibleRust(before), newCode = visibleRust(after);
  const marker = /^\s*#\[\s*(?:cfg\(\s*test\s*\)|test)\s*\]|^\s*mod\s+tests\s*\{/m;
  const oldTest = marker.exec(oldCode), newTest = marker.exec(newCode);
  const fail = () => { throw new Error("Automatic improvement cannot rewrite, remove, enclose or disable existing tests"); };
  if (Boolean(oldTest) !== Boolean(newTest)) fail();
  if (!oldTest) return;
  if (before.slice(oldTest.index) !== after.slice(newTest.index)) fail();
  // New cfg/ignore attributes, imported macro overrides and macro definitions can
  // disable unchanged tests. Require manual review for those edits anywhere in a file.
  const directives = (text, code) => [...code.matchAll(/#!?\[[\s\S]*?\]|\b(?:use|extern\s+crate)\s+[^;]+;|\bmacro_rules\s*!\s*\w+|\bcfg\s*!\s*\([^)]*\)/g)]
    .map(match => text.slice(match.index, match.index + match[0].length));
  if (JSON.stringify(directives(before, oldCode)) !== JSON.stringify(directives(after, newCode))) fail();
  const enclosure = code => {
    const stack = [];
    for (const char of code) {
      if ("{([".includes(char)) stack.push(char);
      else if ("})]".includes(char) && stack.pop() !== ({ "}": "{", ")": "(", "]": "[" })[char]) fail();
    }
    return stack.join("");
  };
  if (enclosure(oldCode.slice(0, oldTest.index)) !== enclosure(newCode.slice(0, newTest.index))) fail();
}
