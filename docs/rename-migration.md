# Rebrand redeploy — operator checklist (Ficina → alo)

Run these **once**, on the production host, at the next deliberate redeploy after
pulling the rebranded code. Nothing on the running server changes until you do.
See [ADR 0016](decisions/0016-rebrand-ficina-to-alo.md) for the why.

> Assumes the compose dir is `$COMPOSE` and the checkout still holds the live
> `.env`. Adjust paths to your host.

```sh
COMPOSE=/opt/alo/deploy/production      # was /opt/ficina/deploy/production
```

## 1. Stop the OLD project — KEEP THE DATA (no `-v`)

```sh
# The old containers ran under compose project "ficina". Stop them so the new
# "alo" project can take the ports. Do NOT pass -v: that would delete volumes.
docker compose -p ficina -f "$COMPOSE/docker-compose.yml" down
```

## 2. Rewrite the live `.env`: FICINA_* → ALO_*, and pin the DB name/role

```sh
cd "$COMPOSE"
cp .env .env.bak.ficina                       # backup first
sed -i -E 's/^FICINA_/ALO_/' .env             # rename every FICINA_* key -> ALO_*

# The database + role are still named "ficina" (rename deferred, ADR 0016).
# Ensure these are pinned so the app connects to the existing data:
grep -q '^POSTGRES_USER=' .env && sed -i 's/^POSTGRES_USER=.*/POSTGRES_USER=ficina/' .env || echo 'POSTGRES_USER=ficina' >> .env
grep -q '^POSTGRES_DB='   .env && sed -i 's/^POSTGRES_DB=.*/POSTGRES_DB=ficina/'     .env || echo 'POSTGRES_DB=ficina'     >> .env
```

## 3. Build the new images and start the new project

```sh
docker compose -f "$COMPOSE/docker-compose.yml" build
docker compose -f "$COMPOSE/docker-compose.yml" up -d      # project name is now "alo"
```

The volumes are pinned to their pre-rebrand names (`ficina_pg_data`,
`ficina_blobs`, `ficina_smtp_spool`, `ficina_certs`, `ficina_caddy_data`), so the
new `alo` services attach to the existing data automatically.

## 4. IF blob/spool permission errors appear (uid changed to 10001)

The app user is now pinned to uid/gid `10001`. If the pre-rebrand volumes were
written by a different uid, fix ownership once (data is untouched):

```sh
for v in ficina_blobs ficina_smtp_spool; do
  docker run --rm -v "$v":/d alpine chown -R 10001:10001 /d
done
docker compose -f "$COMPOSE/docker-compose.yml" restart alo-smtp alo-imap alo-jmap alo-control
```

## 5. Systemd units (backup/monitor) were renamed ficina-* → alo-*

```sh
systemctl disable --now ficina-backup.timer ficina-monitor.timer 2>/dev/null || true
rm -f /etc/systemd/system/ficina-{backup,backup-failed,monitor}.service \
      /etc/systemd/system/ficina-{backup,monitor}.timer
cp "$COMPOSE/ops/systemd/"alo-*.service "$COMPOSE/ops/systemd/"alo-*.timer /etc/systemd/system/
systemctl daemon-reload
systemctl enable --now alo-backup.timer alo-monitor.timer
```

## 6. Verify

```sh
docker compose -f "$COMPOSE/docker-compose.yml" ps          # alo-* containers healthy
docker volume ls | grep ficina_                             # data volumes intact
curl -sfI https://<host>/.well-known/jmap >/dev/null && echo "JMAP ok"
```

## Notes / non-blocking

- **Domain verification**: the DNS TXT prefix is now `_alo-verify` (was
  `_ficina-verify`). Already-verified domains stay verified; only a *new*
  verification needs the new record.
- **Pre-rebrand sent emails** carry `data-ficina-*` attributes; they still render.
  New mail uses `data-alo-*`.
- **Users** are logged out once (the browser refresh-token key changed) and simply
  sign in again.
- **Deferred**: renaming the PostgreSQL database/role from `ficina` to `alo` — do
  it in a maintenance window (dump + restore into an `alo` DB/role, then drop the
  `POSTGRES_*=ficina` pins), and no later than open-sourcing. Tracked in ADR 0016.
```
