# Ficina mail — operations runbook

Plain-language guide to running the live Ficina mail server. It assumes no
deep technical background: every task is "here's what it means, here's the
exact command, here's how you know it worked."

- **Server:** one Linux host at `mail.namel3ss.com`.
- **What it does:** receives mail, lets a mail app (Thunderbird, Apple Mail, a
  phone) read it, sends mail, and provides secure login (OpenID Connect).
- **You reach it with:** `ssh root@mail.namel3ss.com` using your SSH key
  (password login is disabled on purpose).
- **Everything runs as containers** in `/opt/ficina/deploy/production`. Run the
  `docker compose …` commands from inside that directory.

> Golden rule: you almost never need to touch this. It backs itself up,
> patches itself, renews its own certificate, and emails you if something is
> wrong. This runbook is for the rare day something needs a human.

---

## 1. Is everything healthy?

```sh
cd /opt/ficina/deploy/production
docker compose ps
```

Every row should say **running** and the mail services should say
**(healthy)**. That's the whole health check.

To watch what a service is doing (no secrets are ever logged):

```sh
docker compose logs -f ficina-smtp     # or ficina-imap, ficina-jmap, caddy
```

The server also checks itself every 10 minutes and **emails you** if anything
is wrong, so a quiet inbox means a healthy server.

---

## 2. The alert emails — what each one means

Alerts arrive from the server itself (subject starts with `[Ficina]`). You get
one when a problem appears, a reminder at most every 6 hours while it lasts,
and a "recovered" note when it clears.

| Alert says | Plain meaning | What to do |
|---|---|---|
| `service '…' is not running` | A part of the stack stopped. | `docker compose up -d` to bring it back; then `docker compose logs <service>` to see why. |
| `root disk is NN% full` | The disk is filling up. | See §7 (disk). Usually old logs or backups. |
| `TLS certificate expires in N days` | The HTTPS/mail certificate is close to expiry and didn't auto-renew. | See §5 (certificate). |
| `latest backup is Nh old` | The nightly backup didn't run. | `systemctl start ficina-backup.service`, then check `journalctl -u ficina-backup.service`. |
| `NN failed logins in the last …` | Someone is guessing passwords. | Usually harmless (attempts are rate-limited and IPs get banned). If it's you locked out, wait a few minutes. |
| `BACKUP FAILED` | The backup job errored out. | `journalctl -u ficina-backup.service -n50` to see the error. |

Prove alerting still works at any time (sends you a harmless test email):

```sh
python3 /opt/ficina/monitoring/monitor.py --test
```

---

## 3. Add a mailbox or an alias

**A whole new mailbox** (its own login and inbox):

```sh
cd /opt/ficina/deploy/production
docker compose exec -e FICINA_ADMIN_PASSWORD='a-strong-password' \
  ficina-jmap identityctl bootstrap-admin your-org newuser@namel3ss.com
```

**An alias** (a second address that drops into an existing mailbox — e.g.
`dmarc@` or `sales@` landing in your inbox). Find the tenant and user id, then
add the alias:

```sh
docker compose exec -T postgres psql -U ficina -d ficina -c \
  "SELECT id AS user_id, tenant_id, email FROM users;"

# then (replace the address and the two ids):
docker compose exec -T postgres psql -U ficina -d ficina <<'SQL'
INSERT INTO aliases (address, tenant_id, user_id)
VALUES ('sales@namel3ss.com', '<tenant_id>', '<user_id>')
ON CONFLICT (address) DO NOTHING;
SQL
```

Mail to the alias is delivered to that user's inbox. (A friendlier
`identityctl add-alias` command is a future improvement; today it's this
one-line insert.)

---

## 4. Restore from backup

Backups are **encrypted** with restic and run nightly at 03:30. They contain
the database, all message bodies, the TLS certificate, and the config/DKIM
key. **The restic password is required to read them** — it is stored at
`/root/.config/ficina/restic-password` on the server *and* you were told to
keep a copy somewhere off the server. Without it, backups cannot be decrypted
(by you or anyone else — that's the point).

**See what backups exist:**

```sh
export RESTIC_REPOSITORY=/opt/ficina/backups/restic
export RESTIC_PASSWORD_FILE=/root/.config/ficina/restic-password
restic snapshots --tag ficina
```

**Restore the database** (e.g. after data loss). This replaces the live
database — be sure:

```sh
cd /opt/ficina/deploy/production
# 1. pull the newest DB dump out of the backup into /tmp/restore
restic restore latest --tag ficina --include '*/ficina-db.dump' --target /tmp/restore
# 2. load it back in
DUMP=$(find /tmp/restore -name ficina-db.dump)
docker compose exec -T postgres pg_restore -U ficina -d ficina --clean --if-exists < "$DUMP"
```

**Restore message bodies / certs** (files): `restic restore latest --target /tmp/restore`
puts everything back under `/tmp/restore`; copy what you need into the matching
Docker volume mount-point (`docker volume inspect ficina_blobs -f '{{.Mountpoint}}'`).

> This exact restore path has been tested end-to-end into a scratch database —
> the data (users, messages, mailboxes, blobs, certs, DKIM key) came back
> intact. Re-test it after any big change; a backup you've never restored is a
> hope, not a backup.

**Off-server copy:** not yet enabled (needs an external storage account). Once
it is, `backup.sh` gets a `restic copy` step to a second repository, and
you'll restore with the same commands pointed at that repository. Until then,
a total loss of this one server would lose the backups with it — this is the
top open item.

---

## 5. The TLS certificate

One Let's Encrypt certificate covers HTTPS (443) and mail TLS (465/587/993/995).
It **renews itself** automatically (the `certbot` container checks twice a day
and renews within 30 days of expiry).

The mail services pick up a renewed certificate when they restart, so once a
month (or after a renewal) give them a nudge:

```sh
cd /opt/ficina/deploy/production
docker compose restart ficina-smtp ficina-imap
```

If the "certificate expires soon" alert ever fires, do the restart above; if it
persists, check `docker compose logs certbot`.

---

## 6. Security posture (what's already protecting you)

- **Firewall (ufw):** only the ports mail needs are open — 22 (SSH), 25, 80,
  443, 465, 587, 993, 995. Cleartext-capable IMAP **143 is closed**.
- **SSH:** key-only, password login disabled.
- **fail2ban:** automatically bans IPs that repeatedly fail SSH.
- **Automatic security updates:** the OS installs security patches on its own
  and reboots at 04:30 if a patch needs it.
- **Not an open relay:** the server refuses to forward mail for strangers
  (tested). Sending requires a real login.
- **Inbound DMARC enforced:** forged mail from domains that publish a reject
  policy (e.g. a fake "google.com") is refused.
- **Login rate-limiting:** repeated bad logins to mail are slowed with
  per-account backoff and per-connection caps.
- **Logs are capped** at 50 MB per service (rotated) so they can't fill the
  disk, while keeping a generous window of security events.

Check the firewall or the ban list anytime:

```sh
ufw status verbose
fail2ban-client status sshd
```

---

## 7. Disk, and turning things off

**Disk usage:**

```sh
df -h /
docker system df          # how much Docker is using
```

If it's ever tight: old backups already self-prune (7 daily + 4 weekly kept);
container logs are capped. `docker system prune` removes unused images.

**Kill-switch for outbound sending** (stop the server sending mail, keep
receiving/reading):

```sh
# in .env set FICINA_SMTP_OUTBOUND_ENABLED=false, then:
docker compose up -d ficina-smtp
```

**Stop / start everything:**

```sh
docker compose stop        # stop (data kept)
docker compose up -d       # start again
# NEVER: docker compose down -v   ← the -v deletes all data irreversibly
```

---

## 8. DNS records (what's set, what's still to add)

Current, working records for `namel3ss.com`:

| Record | Purpose |
|---|---|
| `MX → mail.namel3ss.com` | where the world sends your mail |
| `A mail.namel3ss.com → 152.53.179.142` | the server |
| `PTR (reverse DNS) → mail.namel3ss.com` | sender reputation |
| `TXT` SPF `v=spf1 mx -all` | only this server may send as you |
| `TXT fic._domainkey` (Ed25519 DKIM) | signs your outgoing mail |
| `TXT _dmarc` `p=quarantine; rua=mailto:dmarc@namel3ss.com` | anti-forgery + reports (now land in your inbox) |

**Still to add** for full deliverability hardening (needs your DNS provider;
none of these cost anything). After you add the first one, ask to have the
certificate re-issued to include `mta-sts.namel3ss.com` and the policy turned on:

| Record | Value |
|---|---|
| `A mta-sts.namel3ss.com` | `152.53.179.142` |
| `TXT _mta-sts.namel3ss.com` | `v=STSv1; id=20260729000000` |
| `TXT _smtp._tls.namel3ss.com` | `v=TLSRPTv1; rua=mailto:dmarc@namel3ss.com` |

MTA-STS tells other mail servers to always use encryption when sending to you
(and never silently fall back to cleartext); TLS-RPT emails you a report if any
of them can't.

---

## 9. Open items (need an account or your action)

1. **Off-server backup copy** — pick EU object storage (Scaleway free tier
   recommended), then the nightly backup gets copied off-box. Until done, the
   backups live only on the server. *(§4 above.)*
2. **External uptime check** — a monitor outside this server so you're alerted
   even if the whole box goes down. Needs a small external account
   (e.g. a free uptime service).
3. **MTA-STS / TLS-RPT DNS** — the three records in §8.
4. **Re-run mail-tester** — after the DNS items, send a message to a fresh
   `mail-tester.com` address to confirm the score. Last score: 8/10, with
   SPF, DKIM and DMARC all passing.
