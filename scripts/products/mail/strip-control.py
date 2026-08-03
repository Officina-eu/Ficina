#!/usr/bin/env python3
"""Remove the suite-only control plane from an exported alomails deploy.

The alomails product ships mail only, so its single-server compose has no
`alo-control` service and its Caddyfile no `/control/*` route. This strips both
from a copied `deploy/production/` — the only deploy transform the export needs.

Usage: strip-control.py <docker-compose.yml> <Caddyfile>
"""
import re
import sys

compose, caddyfile = sys.argv[1], sys.argv[2]

s = open(compose, encoding="utf-8").read()
# Caddy no longer waits on the control service.
s = re.sub(r"\n      alo-control: \{ condition: service_started \}", "", s)
# Drop the control-plane comment and the whole alo-control service, up to the
# next service (caddy).
s = re.sub(
    r"\n  # The multi-tenant control plane \(ADR 0012\).*?(?=\n  caddy:)",
    "",
    s,
    flags=re.S,
)
open(compose, "w", encoding="utf-8").write(s)

c = open(caddyfile, encoding="utf-8").read()
# Drop the @control matcher + its handle block (and the comment above it).
c = re.sub(
    r"\n\t# The multi-tenant control plane.*?\n\t@control path /control/\*\n"
    r"\thandle @control \{\n\t\treverse_proxy alo-control:8090\n\t\}\n",
    "\n",
    c,
    flags=re.S,
)
open(caddyfile, "w", encoding="utf-8").write(c)

# Fail loudly if anything control-shaped survived — a silent leak of a
# suite-only service into the public repo is worse than a broken export.
for path in (compose, caddyfile):
    text = open(path, encoding="utf-8").read()
    if "alo-control" in text:
        sys.exit(f"strip-control: 'alo-control' still present in {path}")

print("stripped control plane from compose + Caddyfile")
