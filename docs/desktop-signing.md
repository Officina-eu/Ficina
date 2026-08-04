# Desktop app: update feed + code-signing

The alomails desktop app has **two independent signatures**, often confused:

1. **Updater signing** (already in place). A minisign key proves an update came
   from us; the app refuses any update its baked-in public key can't verify.
   Nothing to buy — the keypair was generated with `cargo tauri signer generate`
   and its private half is the `TAURI_SIGNING_PRIVATE_KEY` CI secret.
2. **OS code-signing** (this doc). What removes the operating system's
   "unidentified developer" / SmartScreen warning on install. This needs paid
   certificates from Apple and a Windows signing authority.

The release workflow (`.github/workflows/desktop-release.yml`) is already wired
for all of this — it lights up as each secret is added.

---

## 1. The sovereign update feed (enables macOS auto-update)

Every release, CI publishes `latest.json` + the installers + updater packages to
our own server at `alomails.com/download`, which the apps poll. This is what
makes **macOS** (and Windows) update themselves without hosting on a third party.

Add two secrets:

| Secret | Value |
|---|---|
| `ALO_DEPLOY_HOST` | `user@<server>` for the publish target |
| `ALO_DEPLOY_KEY` | that user's SSH **private** key |

**Use a restricted key, not the root deploy key.** CI should not hold root.
Create a dedicated key whose access is limited to the downloads directory — e.g.
a user that owns only `/opt/alo/deploy/production/downloads`, or an
`authorized_keys` entry with a forced `rsync`/`scp` command. Then CI can publish
updates but nothing else.

Without these secrets the workflow still builds and attaches everything to the
draft GitHub release; it just skips the server publish.

---

## 2. macOS — Developer ID + notarization

1. Enrol in the **Apple Developer Program** ($99/yr).
2. Create a **Developer ID Application** certificate (Keychain Access → Certificate
   Assistant, or developer.apple.com → Certificates). Export it as a `.p12` with
   a password.
3. Create notarization credentials: an **App Store Connect API key**, or an
   app-specific password for your Apple ID.

Add these secrets (the workflow's macOS leg passes them to `tauri-action`, which
then signs **and** notarizes automatically):

| Secret | Value |
|---|---|
| `APPLE_CERTIFICATE` | base64 of the `.p12` (`base64 -i cert.p12`) |
| `APPLE_CERTIFICATE_PASSWORD` | the `.p12` password |
| `APPLE_SIGNING_IDENTITY` | e.g. `Developer ID Application: Your Name (TEAMID)` |
| `APPLE_ID` | your Apple ID email |
| `APPLE_PASSWORD` | app-specific password |
| `APPLE_TEAM_ID` | your 10-char team id |

That's the whole macOS story — no `tauri.conf` change needed.

---

## 3. Windows — a cloud signing service

Since June 2023 the CA/Browser Forum requires code-signing private keys to live
on FIPS hardware/HSM, so a `.pfx` in a CI secret is no longer allowed for new
certs. The CI-friendly path is a cloud signing service that holds the key and
signs on request. Recommended: **Azure Trusted Signing** (~$10/mo, identity
verification required).

**Important ordering:** the OS signature must be applied *during* the Tauri
build, not after — Tauri computes the updater signature over the final bytes, so
signing the `.exe` afterward would invalidate the update. Tauri supports this via
a **`signCommand`**. Once you have an Azure Trusted Signing account, add to
`apps/desktop/src-tauri/tauri.conf.json` under `bundle`:

```jsonc
"windows": {
  "signCommand": "trusted-signing-cli -e <endpoint> -a <account> -c <cert-profile> %1"
}
```

and provide the Azure credentials to CI as secrets (client id / tenant /
secret), which the tool reads. Alternatives if you prefer: an **EV cert on a USB
token** (can't run in CI — you'd sign locally) or **SSL.com eSigner** /
**DigiCert KeyLocker** (also cloud, similar shape).

Until this is set up, Windows installs work but show a SmartScreen
"unrecognized app" prompt (More info → Run anyway).

---

## Cutting a signed release

Once the secrets are in: bump `version` in `apps/desktop/src-tauri/tauri.conf.json`
and `Cargo.toml`, commit, then push a tag:

```sh
git tag -a desktop-v0.1.2 -m "alomails desktop 0.1.2"
git push origin desktop-v0.1.2
```

CI builds both platforms, signs them, and publishes the feed — installed apps
pick up the update on next launch.
