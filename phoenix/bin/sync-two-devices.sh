#!/usr/bin/env bash
# Two devices syncing over the HTTP endpoints, with curl: device A
# writes a post, device B pulls it, both edit it, and the server keeps the
# later edit. The "Delivers" check of phase 4 in docs/design/build-order.md.
#
#   SYNC_URL=http://localhost:4000 SYNC_TOKEN=... SYNC_USER_ID=... \
#   bin/sync-two-devices.sh
#
# The server must run with API_SOCKET=true. The token is an API token of
# the user, see App.Data.Accounts.generate_user_api_token/1.
set -euo pipefail

: "${SYNC_URL:=http://localhost:4000}"
: "${SYNC_TOKEN:?set SYNC_TOKEN to an API token of the user}"
: "${SYNC_USER_ID:?set SYNC_USER_ID to the id of the user}"

sync() {
  curl -sS -X POST "$SYNC_URL/sync/$1" \
    -H "Authorization: Bearer $SYNC_TOKEN" -H "Content-Type: application/json" -d "$2"
}

stamp() { date -u +%Y-%m-%dT%H:%M:%S.000000Z; }
post_row() { printf '{"table":"app_posts","row":{"id":"%s","body":"%s","author_id":"%s","updated_at":"%s"}}' "$1" "$2" "$SYNC_USER_ID" "$3"; }

id=$(uuidgen | tr '[:upper:]' '[:lower:]')
written_at=$(stamp)

echo "device A writes a post"
sync push "{\"rows\":[$(post_row "$id" "from device A" "$written_at")]}"; echo

echo "device B pulls it"
sync pull '{}' | grep -o "\"body\":\"from device A\"" || { echo "device B did not receive the post"; exit 1; }

sleep 1
echo "device B edits it, later"
sync push "{\"rows\":[$(post_row "$id" "edited on B" "$(stamp)")]}"; echo

echo "device A edits it too, but with its older time, and is told the server's version"
sync push "{\"rows\":[$(post_row "$id" "edited on A" "$written_at")]}" | grep -o '"reason":"stale"' || { echo "the stale edit was not refused"; exit 1; }

echo "both pull and match the server"
sync pull '{}' | grep -o "\"body\":\"edited on B\"" >/dev/null && echo "ok: both devices end with 'edited on B'"
