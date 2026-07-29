// The AI-provider catalog the admin screen presents. Only kinds our inference
// layer can actually drive are listed (all speak the OpenAI-compatible Chat
// Completions contract): self-hosted Ollama, OpenAI, and a custom endpoint.
// Anthropic (native, non-OpenAI API) and a hosted "Ficina AI" are not shown
// until their backends exist — no card that can't work.
import { strings } from "../i18n";

export interface CatalogEntry {
  kind: string;
  name: string;
  description: string;
  group: "self" | "keys";
  defaultBaseUrl: string;
  needsKey: boolean;
}

export const CATALOG: CatalogEntry[] = [
  {
    kind: "ollama",
    name: strings.kindOllama,
    description: strings.ollamaDesc,
    group: "self",
    defaultBaseUrl: "http://localhost:11434",
    needsKey: false,
  },
  {
    kind: "openai",
    name: strings.kindOpenai,
    description: strings.openaiDesc,
    group: "keys",
    defaultBaseUrl: "https://api.openai.com/v1",
    needsKey: true,
  },
  {
    kind: "custom",
    name: strings.kindCustom,
    description: strings.customDesc,
    group: "keys",
    defaultBaseUrl: "",
    needsKey: true,
  },
];
