// Add or edit an AI provider (admin). Presets an endpoint per kind, lets the
// admin test connectivity before saving, and on save enables the provider (and
// makes it the default when the tenant has none yet).
import { useState } from "react";
import type { FormEvent } from "react";
import { Check, X } from "lucide-react";

import { strings } from "../i18n";
import { Button, Spinner } from "../ds";
import { useJmapClient } from "../jmap";
import type { AiProvider } from "../jmap";
import styles from "./admin.module.css";

interface KindPreset {
  kind: string;
  label: string;
  baseUrl: string;
  needsKey: boolean;
}

const KINDS: KindPreset[] = [
  { kind: "ollama", label: strings.kindOllama, baseUrl: "http://localhost:11434", needsKey: false },
  { kind: "openai", label: strings.kindOpenai, baseUrl: "https://api.openai.com", needsKey: true },
  { kind: "custom", label: strings.kindCustom, baseUrl: "", needsKey: true },
];

interface ProviderModalProps {
  provider?: AiProvider;
  /** True when the tenant has no default yet, so a save should set this one. */
  makeDefaultOnSave: boolean;
  onClose: () => void;
  onSaved: () => void;
}

export function ProviderModal({ provider, makeDefaultOnSave, onClose, onSaved }: ProviderModalProps) {
  const client = useJmapClient();
  const editing = provider !== undefined;
  const [kind, setKind] = useState(provider?.kind ?? "ollama");
  const preset = KINDS.find((k) => k.kind === kind) ?? KINDS[2]!;

  const [baseUrl, setBaseUrl] = useState(provider?.baseUrl ?? preset.baseUrl);
  const [model, setModel] = useState(provider?.model ?? "");
  const [apiKey, setApiKey] = useState("");
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ ok: boolean; models: number } | "fail" | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  function pickKind(next: string) {
    setKind(next);
    if (!editing) {
      const p = KINDS.find((k) => k.kind === next);
      if (p !== undefined) setBaseUrl(p.baseUrl);
    }
    setTestResult(null);
  }

  async function test() {
    if (baseUrl.trim().length === 0 || testing) return;
    setTesting(true);
    setTestResult(null);
    try {
      const res = await client.testConnection(baseUrl.trim(), apiKey.trim());
      setTestResult(res);
    } catch {
      setTestResult("fail");
    } finally {
      setTesting(false);
    }
  }

  async function save(e: FormEvent) {
    e.preventDefault();
    if (baseUrl.trim().length === 0 || model.trim().length === 0) {
      setError(strings.providerRequired);
      return;
    }
    setSaving(true);
    setError(null);
    const id = provider?.id ?? crypto.randomUUID();
    try {
      await client.upsertProvider({
        id,
        kind,
        label: preset.label,
        baseUrl: baseUrl.trim(),
        model: model.trim(),
        enabled: true,
        ...(apiKey.trim().length > 0 ? { apiKey: apiKey.trim() } : {}),
      });
      if (makeDefaultOnSave) await client.setDefaultProvider(id);
      onSaved();
    } catch {
      setError(strings.providerSaveError);
      setSaving(false);
    }
  }

  return (
    <div className={styles.overlay} onMouseDown={onClose}>
      <div
        className={styles.modal}
        role="dialog"
        aria-modal="true"
        aria-label={editing ? strings.providerEdit : strings.providerNew}
        onMouseDown={(e) => e.stopPropagation()}
      >
        <form onSubmit={save}>
          <div className={styles.modalHead}>
            <h2>{editing ? strings.providerEdit : strings.providerNew}</h2>
            <button type="button" className={styles.iconBtn} onClick={onClose} aria-label={strings.composeDiscard}>
              <X size={18} />
            </button>
          </div>

          <div className={styles.modalBody}>
            {!editing && (
              <label className={styles.field}>
                <span className={styles.label}>{strings.providerKind}</span>
                <select className={styles.input} value={kind} onChange={(e) => pickKind(e.target.value)}>
                  {KINDS.map((k) => (
                    <option key={k.kind} value={k.kind}>
                      {k.label}
                    </option>
                  ))}
                </select>
              </label>
            )}

            <label className={styles.field}>
              <span className={styles.label}>{strings.providerBaseUrl}</span>
              <input
                className={styles.input}
                value={baseUrl}
                onChange={(e) => {
                  setBaseUrl(e.target.value);
                  setTestResult(null);
                }}
                placeholder="http://localhost:11434"
              />
            </label>

            <label className={styles.field}>
              <span className={styles.label}>{strings.providerModel}</span>
              <input
                className={styles.input}
                value={model}
                onChange={(e) => setModel(e.target.value)}
                placeholder="llama3.2"
              />
            </label>

            <label className={styles.field}>
              <span className={styles.label}>{strings.providerApiKey}</span>
              <input
                className={styles.input}
                type="password"
                value={apiKey}
                onChange={(e) => setApiKey(e.target.value)}
                placeholder={
                  provider?.hasKey === true
                    ? strings.providerApiKeyKept
                    : preset.needsKey
                      ? "sk-…"
                      : strings.providerApiKeyOptional
                }
              />
            </label>

            <div className={styles.testRow}>
              <button type="button" className={styles.testBtn} onClick={() => void test()} disabled={testing}>
                {testing ? <Spinner size={14} /> : null}
                <span>{testing ? strings.providerTesting : strings.providerTest}</span>
              </button>
              {testResult !== null && testResult !== "fail" && testResult.ok && (
                <span className={styles.testOk}>
                  <Check size={15} /> {strings.providerTestOk(testResult.models)}
                </span>
              )}
              {testResult === "fail" && <span className={styles.testFail}>{strings.providerTestFail}</span>}
            </div>

            {error !== null && (
              <p className={styles.error} role="alert">
                {error}
              </p>
            )}
          </div>

          <div className={styles.modalFoot}>
            <button type="button" className={styles.textBtn} onClick={onClose}>
              {strings.composeDiscard}
            </button>
            <Button type="submit" disabled={saving}>
              {saving ? <Spinner size={16} /> : strings.providerSave}
            </Button>
          </div>
        </form>
      </div>
    </div>
  );
}
