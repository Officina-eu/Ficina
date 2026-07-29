// Admin — AI providers. Lists the tenant's configured backends, lets an admin
// enable/disable each, pick the default the AI features use, edit, or delete,
// and add a new one. All writes go through the admin-gated /admin/ai routes.
import { useCallback, useEffect, useState } from "react";
import { Plus, Server, Star, Trash2 } from "lucide-react";

import { strings } from "../i18n";
import { Spinner, cx } from "../ds";
import { useJmapClient } from "../jmap";
import type { AiProvider } from "../jmap";
import { ProviderModal } from "./ProviderModal";
import styles from "./admin.module.css";

type Editing = { provider?: AiProvider } | null;

export function AiProvidersPage() {
  const client = useJmapClient();
  const [providers, setProviders] = useState<AiProvider[] | null>(null);
  const [error, setError] = useState(false);
  const [editing, setEditing] = useState<Editing>(null);

  const load = useCallback(() => {
    setError(false);
    client
      .listProviders()
      .then(setProviders)
      .catch(() => setError(true));
  }, [client]);

  useEffect(load, [load]);

  const hasDefault = (providers ?? []).some((p) => p.isDefault && p.enabled);

  async function toggle(p: AiProvider) {
    // Optimistic; reload reconciles.
    setProviders((prev) =>
      (prev ?? []).map((x) => (x.id === p.id ? { ...x, enabled: !x.enabled } : x)),
    );
    try {
      await client.upsertProvider({
        id: p.id,
        kind: p.kind,
        label: p.label,
        baseUrl: p.baseUrl,
        model: p.model,
        enabled: !p.enabled,
      });
    } finally {
      load();
    }
  }

  async function makeDefault(p: AiProvider) {
    await client.setDefaultProvider(p.id);
    load();
  }

  async function remove(p: AiProvider) {
    await client.deleteProvider(p.id);
    load();
  }

  return (
    <div className={styles.page}>
      <header className={styles.pageHead}>
        <div>
          <h1>{strings.adminAiProviders}</h1>
          <p className={styles.pageIntro}>{strings.adminAiIntro}</p>
        </div>
        <button type="button" className={styles.primary} onClick={() => setEditing({})}>
          <Plus size={16} />
          <span>{strings.adminAddProvider}</span>
        </button>
      </header>

      {providers === null && !error && (
        <div className={styles.state}>
          <Spinner size={22} />
        </div>
      )}
      {error && (
        <div className={styles.state}>
          <p>{strings.adminProvidersError}</p>
          <button type="button" className={styles.textBtn} onClick={load}>
            {strings.mailRetry}
          </button>
        </div>
      )}

      {providers !== null && providers.length === 0 && (
        <div className={styles.empty}>{strings.adminNoProviders}</div>
      )}

      {providers !== null && providers.length > 0 && (
        <ul className={styles.providerList}>
          {providers.map((p) => (
            <li key={p.id} className={cx(styles.provider, p.isDefault && p.enabled && styles.providerDefault)}>
              <span className={styles.providerIcon}>
                <Server size={20} strokeWidth={1.75} />
              </span>
              <div className={styles.providerText}>
                <div className={styles.providerName}>
                  <strong>{p.label}</strong>
                  {p.isDefault && p.enabled && (
                    <span className={styles.defaultBadge}>{strings.adminDefaultBadge}</span>
                  )}
                </div>
                <div className={styles.providerMeta}>
                  {p.baseUrl} · {p.model}
                </div>
              </div>

              <div className={styles.providerActions}>
                {p.enabled && !p.isDefault && (
                  <button type="button" className={styles.ghost} onClick={() => void makeDefault(p)}>
                    <Star size={15} />
                    <span>{strings.adminMakeDefault}</span>
                  </button>
                )}
                <button type="button" className={styles.ghost} onClick={() => setEditing({ provider: p })}>
                  {strings.adminManage}
                </button>
                <button
                  type="button"
                  className={styles.iconBtn}
                  onClick={() => void remove(p)}
                  aria-label={strings.delete}
                >
                  <Trash2 size={16} />
                </button>
                <label className={styles.toggle}>
                  <input type="checkbox" checked={p.enabled} onChange={() => void toggle(p)} />
                  <span className={styles.track} />
                </label>
              </div>
            </li>
          ))}
        </ul>
      )}

      {editing !== null && (
        <ProviderModal
          {...(editing.provider !== undefined ? { provider: editing.provider } : {})}
          makeDefaultOnSave={!hasDefault}
          onClose={() => setEditing(null)}
          onSaved={() => {
            setEditing(null);
            load();
          }}
        />
      )}
    </div>
  );
}
