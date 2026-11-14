import { fields } from "./workshop-process.mjs";

export function runWorkshopCampaign(service, request) {
  fields(request, ["version", "budget", "candidates"]);
  fields(request.budget, ["maxCandidates", "maxStages"]);
  const { maxCandidates, maxStages } = request.budget;
  if (!Number.isSafeInteger(maxCandidates) || maxCandidates < 1 || maxCandidates > 3
    || !Number.isSafeInteger(maxStages) || maxStages < 1 || maxStages > maxCandidates
    || !Array.isArray(request.candidates) || request.candidates.length > 12) throw new Error("Campaign budget: 1-3 candidates and no more stages than candidates.");
  const ids = new Set();
  for (const candidate of request.candidates) {
    fields(candidate, ["id", "requirements", "operations"]);
    if (typeof candidate.id !== "string" || !/^[a-z][a-z0-9-]{0,63}$/.test(candidate.id) || ids.has(candidate.id)) {
      throw new Error("Campaign candidates need unique stable IDs.");
    }
    ids.add(candidate.id);
  }
  const result = { stop: "no-remaining-candidate", attempted: 0, staged: 0, modelCalls: 0, tokens: 0, outcomes: [] };
  const seen = new Set();
  for (const candidate of request.candidates) {
    if (result.attempted >= maxCandidates || result.staged >= maxStages) { result.stop = "budget-exhausted"; break; }
    result.attempted++;
    const command = { version: request.version, requirements: candidate.requirements, operations: candidate.operations };
    try {
      const plan = service.preview(command);
      if (plan.status === "authority-required") {
        result.outcomes.push({ id: candidate.id, status: plan.status, gaps: plan.capabilities.gaps });
        result.stop = "authority-required";
        break;
      }
      if (plan.status === "no-change" || seen.has(plan.candidate)) {
        result.outcomes.push({ id: candidate.id, status: "no-change" });
        continue;
      }
      const staged = service.stage({ ...command, approval: plan.approval });
      seen.add(plan.candidate);
      result.staged++;
      result.outcomes.push({ id: candidate.id, status: staged.status, directory: staged.directory, evidence: staged.manifest.evidence });
    } catch (error) {
      result.outcomes.push({ id: candidate.id, status: "candidate-failed", error: error.message });
      result.stop = "candidate-failed";
      break;
    }
  }
  return result;
}

