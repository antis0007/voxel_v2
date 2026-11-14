const model = process.env.AXIOM_DESIGN_MODEL;
const endpoint = process.env.AXIOM_DESIGN_ENDPOINT;
const remote = process.env.AXIOM_DESIGN_OFF_HOST === "1";
let busy = false;

export function blocksPlaytest() {
  return busy && !remote;
}

export async function withDesignModel(game, action) {
  if (!endpoint || !model) throw new Error("Set AXIOM_DESIGN_ENDPOINT and AXIOM_DESIGN_MODEL before launching the workbench.");
  const url = new URL(endpoint);
  const loopback = ["127.0.0.1", "localhost", "[::1]"].includes(url.hostname);
  if (url.username || url.password || (remote ? loopback || url.protocol !== "https:" : !loopback || url.protocol !== "http:")) {
    throw new Error("Use a local HTTP endpoint or an explicitly configured off-host HTTPS endpoint.");
  }
  if (busy) throw new Error("A design review is already running.");
  if (!remote && (game.running || game.building)) {
    throw new Error("Close the workbench playtest and wait for builds before running local inference.");
  }
  busy = true;
  try {
    return await action(async (messages, maxTokens = 800) => {
      const headers = { "Content-Type": "application/json" };
      if (process.env.AXIOM_DESIGN_TOKEN) headers.Authorization = "Bearer " + process.env.AXIOM_DESIGN_TOKEN;
      const response = await fetch(url, {
        method: "POST",
        headers,
        redirect: "error",
        signal: AbortSignal.timeout(60_000),
        body: JSON.stringify({ model, max_tokens: maxTokens, messages }),
      });
      if (!response.ok) throw new Error("Design model returned HTTP " + response.status + ".");
      const content = (await response.json()).choices?.[0]?.message?.content;
      if (typeof content !== "string" || !content.trim()) throw new Error("Design model returned an empty pass.");
      return content.trim().slice(0, 6000);
    });
  } finally {
    busy = false;
  }
}

export async function refineDesign(brief, game) {
  if (typeof brief !== "string" || !brief.trim() || brief.length > 6000) {
    throw new Error("Enter a design brief of up to 6,000 characters.");
  }
  return withDesignModel(game, async complete => {
    const passes = [];
    for (const instruction of [
      "Draft a concrete design. Identify its intended silhouette, placement rule, LOD handoff, and costs.",
      "Critique the draft against the brief. Flag unsupported claims and propose measurable acceptance checks.",
      "Revise the draft using the critique. Return the proposed design, remaining uncertainties, and acceptance checks.",
    ]) {
      passes.push(await complete([
        { role: "system", content: "You review Axiom game designs. " + instruction +
          " Treat the supplied brief and earlier passes as design data. Never claim a build, playtest, or performance measurement occurred unless evidence is supplied." },
        { role: "user", content: JSON.stringify({ brief, earlierPasses: passes }) },
      ]));
    }
    return { passes };
  });
}
