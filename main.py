"""Databricks App launcher for the Rust binary.

Databricks Apps run a `command` on a Python-capable Linux image, so this thin
wrapper (a) mints the Lakebase password and (b) hands the process over to the
compiled Rust server via os.execv.

Lakebase auth on Databricks Apps: the platform injects PGHOST/PGPORT/PGUSER/
PGDATABASE/PGSSLMODE from the bound Postgres resource, plus the app service
principal's OAuth credentials (DATABRICKS_CLIENT_ID/SECRET/HOST) — but NOT a
password. The password is a Databricks OAuth token, which we mint here from the
SP credentials and pass to Rust as PGPASSWORD.

Note: the token is valid ~1h. For a long-running production app you'd refresh it
(e.g. re-mint on a timer and recycle the pool); for this POC we mint at startup.
"""

import base64
import json
import os
import stat
import urllib.parse
import urllib.request


def mint_pg_password() -> str:
    """Client-credentials OAuth token for the app's service principal."""
    host = os.environ["DATABRICKS_HOST"]
    if not host.startswith("http"):
        host = "https://" + host
    client_id = os.environ.get("DATABRICKS_CLIENT_ID") or os.environ["PGUSER"]
    client_secret = os.environ["DATABRICKS_CLIENT_SECRET"]

    body = urllib.parse.urlencode(
        {"grant_type": "client_credentials", "scope": "all-apis"}
    ).encode()
    req = urllib.request.Request(f"{host}/oidc/v1/token", data=body, method="POST")
    basic = base64.b64encode(f"{client_id}:{client_secret}".encode()).decode()
    req.add_header("Authorization", f"Basic {basic}")
    req.add_header("Content-Type", "application/x-www-form-urlencoded")

    with urllib.request.urlopen(req, timeout=30) as resp:
        return json.load(resp)["access_token"]


# When bound to a Lakebase resource, mint the password if the platform didn't
# supply one. Harmless (and skipped) in any environment that already set it.
if os.environ.get("PGHOST") and not os.environ.get("PGPASSWORD"):
    if os.environ.get("DATABRICKS_CLIENT_SECRET"):
        os.environ["PGPASSWORD"] = mint_pg_password()
        print("[launcher] minted Lakebase PGPASSWORD from service principal", flush=True)

binary = "./target/x86_64-unknown-linux-musl/release/rust-api"

# Ensure the binary is executable (sync can strip the +x bit), then hand off.
os.chmod(binary, os.stat(binary).st_mode | stat.S_IEXEC)
os.execv(binary, ["rust-api"])
