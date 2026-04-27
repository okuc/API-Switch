import rawCatalog from "../../models.json";

export type CatalogModel = {
  id: string;
  name?: string;
  family?: string;
  release_date?: string;
  last_updated?: string;
  open_weights?: boolean;
  reasoning?: boolean;
  tool_call?: boolean;
  structured_output?: boolean;
  attachment?: boolean;
  temperature?: boolean;
  interleaved?: unknown;
  status?: string;
  experimental?: unknown;
  modalities?: {
    input?: string[];
    output?: string[];
  };
  limit?: {
    context?: number;
    input?: number;
    output?: number;
  };
};

type CatalogProvider = {
  id: string;
  name?: string;
  api?: string;
  doc?: string;
  models?: Record<string, CatalogModel>;
};

type CatalogData = Record<string, CatalogProvider>;

const catalog = rawCatalog as CatalogData;
const modelIndex = new Map<string, CatalogModel>();
const modelEntries: Array<{ key: string; normalized: string; model: CatalogModel }> = [];

const removableSuffixes = [
  "free",
  "beta",
  "preview",
  "latest",
  "exp",
  "experimental",
  "thinking",
  "search",
  "online",
];

function scoreModel(model: CatalogModel): number {
  let score = 0;
  if (model.release_date) score += 2;
  if (model.last_updated) score += 1;
  if (model.reasoning) score += 1;
  if (model.tool_call) score += 1;
  if (model.structured_output) score += 1;
  if (model.attachment) score += 1;
  if (model.temperature) score += 1;
  if (model.modalities?.input?.length) score += model.modalities.input.length;
  if (model.modalities?.output?.length) score += model.modalities.output.length;
  if (model.limit?.context) score += 1;
  if (model.limit?.output) score += 1;
  return score;
}

function normalizeModelKey(value: string): string {
  return value
    .trim()
    .toLowerCase()
    .replace(/^[a-z0-9._-]+\//, "")
    .replace(/[._]/g, "-")
    .replace(/-{2,}/g, "-")
    .replace(/-v(\d)(?=-|$)/g, "-$1")
    .replace(/-(20\d{2}|19\d{2})(?=-|$)/g, "")
    .replace(/-(\d{4}-\d{2}-\d{2}|\d{4}-\d{2}|\d{8})$/g, "")
    .replace(/^-+|-+$/g, "");
}

function buildKeyVariants(value: string): string[] {
  const variants = new Set<string>();
  let current = normalizeModelKey(value);
  if (!current) return [];
  variants.add(current);

  while (true) {
    const next = removableSuffixes.find((suffix) => current.endsWith(`-${suffix}`));
    if (!next) break;
    current = current.slice(0, -(next.length + 1)).replace(/-+$/g, "");
    if (!current) break;
    variants.add(current);
  }

  return Array.from(variants);
}

function tokenize(value: string): string[] {
  return value.split(/[^a-z0-9]+/).filter(Boolean);
}

function similarityScore(input: string, candidate: string): number {
  if (input === candidate) return 10_000;
  if (candidate.includes(input)) return 8_000 - (candidate.length - input.length);
  if (input.includes(candidate)) return 7_000 - (input.length - candidate.length);

  const inputTokens = new Set(tokenize(input));
  const candidateTokens = new Set(tokenize(candidate));
  let overlap = 0;
  for (const token of inputTokens) {
    if (candidateTokens.has(token)) overlap += 1;
  }
  if (overlap === 0) return -1;

  const penalty = Math.abs(input.length - candidate.length);
  return overlap * 100 - penalty;
}

for (const provider of Object.values(catalog)) {
  for (const [modelKey, model] of Object.entries(provider.models || {})) {
    const key = modelKey.toLowerCase();
    const current = modelIndex.get(key);
    if (!current || scoreModel(model) > scoreModel(current)) {
      modelIndex.set(key, model);
    }
    if (model.id && model.id.toLowerCase() !== key) {
      const idKey = model.id.toLowerCase();
      const currentById = modelIndex.get(idKey);
      if (!currentById || scoreModel(model) > scoreModel(currentById)) {
        modelIndex.set(idKey, model);
      }
    }

    for (const variant of new Set([key, model.id?.toLowerCase()].filter(Boolean) as string[])) {
      for (const normalized of buildKeyVariants(variant)) {
        modelEntries.push({ key: variant, normalized, model });
      }
    }
  }
}

export function getCatalogModel(modelId: string): CatalogModel | null {
  const key = modelId.trim().toLowerCase();
  if (!key) return null;

  const direct = modelIndex.get(key);
  if (direct) return direct;

  const variants = buildKeyVariants(key);
  for (const variant of variants) {
    const matched = modelIndex.get(variant);
    if (matched) return matched;
  }

  let best: { model: CatalogModel; score: number } | null = null;
  for (const variant of variants) {
    for (const entry of modelEntries) {
      const score = similarityScore(variant, entry.normalized);
      if (score <= 0) continue;
      const weighted = score + scoreModel(entry.model);
      if (!best || weighted > best.score) {
        best = { model: entry.model, score: weighted };
      }
    }
  }

  return best?.score && best.score >= 150 ? best.model : null;
}

export function formatTokenCount(value?: number): string | null {
  if (!value || value <= 0) return null;
  if (value >= 1_000_000) {
    const n = value / 1_000_000;
    return `${Number.isInteger(n) ? n : n.toFixed(1)}M`;
  }
  if (value >= 1_000) {
    const n = value / 1_000;
    return `${Number.isInteger(n) ? n : n.toFixed(1)}K`;
  }
  return String(value);
}
