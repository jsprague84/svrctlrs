#!/usr/bin/env bash
# The svrctlrs client in Pocket ID (the dev-space identity provider), kept here in git instead
# of only in Pocket ID's admin page. Owner deployment only; any other provider is configured
# by hand (docs/DEPLOYMENT.md, "Signing in with an identity provider").
#
#   scripts/deploy/pocket-id-client.sh                 show what would change (writes nothing)
#   scripts/deploy/pocket-id-client.sh --apply         create or update it, then read it back
#   scripts/deploy/pocket-id-client.sh --new-secret    print a new client secret on stdout
#                                                      (the old one stops working); pipe it
#                                                      straight into the deployment's .env
#
# Edge-auth standard: every client is group-restricted, infrastructure ones to `owner` only, so
# a relative's "My Apps" never lists it. Pocket ID's client update is a full PUT (a field left
# out is reset: PKCE off, the restriction lifted), so the body carries every field, and the
# client is compared with what was expected afterwards. Needs the admin key at
# ~/.config/pocket-id/api-key (never in the repo).
set -euo pipefail
ID="${POCKET_ID_URL:-https://id.js-node.cc}"
NAME="${SVRCTLRS_OIDC_CLIENT_NAME:-SvrCtlRS}"
ORIGIN="${SVRCTLRS_ORIGIN:-https://svrctlrs.js-node.app}"
OWNER_GROUP="${POCKET_ID_OWNER_GROUP:-055745c8-2580-4daa-bf2a-6586cd7aa7d0}"
KEY_FILE="${POCKET_ID_KEY_FILE:-$HOME/.config/pocket-id/api-key}"
[ -r "$KEY_FILE" ] || { echo "no Pocket ID key at $KEY_FILE" >&2; exit 2; }
MODE="${1:-show}"

api() { curl -fsS -m 30 -H "X-API-KEY: $(cat "$KEY_FILE")" -H 'content-type: application/json' "$@"; }

# What the client must be (the fields of Pocket ID's OidcClientUpdateDto)
WANT="$(jq -n --arg n "$NAME" --arg o "$ORIGIN" '{
  name: $n,
  description: "SSH terminals on your servers",
  callbackURLs: [($o + "/auth/oidc/callback")],
  logoutCallbackURLs: [],
  isPublic: false,
  pkceEnabled: true,
  requiresReauthentication: false,
  requiresPushedAuthorizationRequests: false,
  skipConsent: false,
  launchURL: ($o + "/auth/oidc/start"),
  isGroupRestricted: true
}')"

CLIENT="$(api "$ID/api/oidc/clients?search=$(jq -rn --arg n "$NAME" '$n|@uri')" \
  | jq -r --arg n "$NAME" '[.data[] | select(.name == $n)] | if length > 1 then "MANY" else (.[0].id // "") end')"
[ "$CLIENT" = MANY ] && { echo "more than one client is called $NAME; fix that by hand" >&2; exit 1; }

if [ "$MODE" = --new-secret ]; then
  [ -n "$CLIENT" ] || { echo "no client $NAME yet; run --apply first" >&2; exit 1; }
  api -X POST "$ID/api/oidc/clients/$CLIENT/secret" | jq -er .secret
  exit 0
fi

# Compare what Pocket ID holds with WANT, plus the allowed group
matches() {
  jq -e --argjson w "$WANT" --arg g "$OWNER_GROUP" '
    (to_entries | map(select(.key as $k | $w | has($k))) | from_entries) == $w
    and ([.allowedUserGroups[].id] == [$g])' >/dev/null
}

if [ -z "$CLIENT" ]; then
  echo "Pocket ID has no client called $NAME — would create:"
  jq . <<<"$WANT"
  echo "allowed group: owner ($OWNER_GROUP)"
  [ "$MODE" = --apply ] || { echo "(nothing written; run with --apply)"; exit 0; }
  CLIENT="$(api -X POST -d "$WANT" "$ID/api/oidc/clients" | jq -er .id)"
  echo "created client $CLIENT"
else
  BEFORE="$(api "$ID/api/oidc/clients/$CLIENT")"
  if matches <<<"$BEFORE"; then
    echo "✓ client $NAME ($CLIENT) is as it should be — nothing to do"; exit 0
  fi
  echo "client $NAME ($CLIENT) — would change:"
  diff <(jq -S --argjson w "$WANT" 'to_entries | map(select(.key as $k | $w | has($k))) | from_entries' <<<"$BEFORE") \
       <(jq -S . <<<"$WANT") || true
  [ "$MODE" = --apply ] || { echo "(nothing written; run with --apply)"; exit 0; }
  # Every field it had, with ours on top (a full PUT resets anything left out)
  BODY="$(jq --argjson w "$WANT" '{name, description, callbackURLs, logoutCallbackURLs, isPublic,
    pkceEnabled, requiresReauthentication, requiresPushedAuthorizationRequests, skipConsent,
    credentials: (.credentials // {}), launchURL, hasLogo, hasDarkLogo, isGroupRestricted} + $w' <<<"$BEFORE")"
  api -X PUT -o /dev/null -d "$BODY" "$ID/api/oidc/clients/$CLIENT"
fi

api -X PUT -o /dev/null -d "$(jq -n --arg g "$OWNER_GROUP" '{userGroupIds: [$g]}')" \
  "$ID/api/oidc/clients/$CLIENT/allowed-user-groups"

# Read it back: an API that ignores a field fails silently
AFTER="$(api "$ID/api/oidc/clients/$CLIENT")"
if ! matches <<<"$AFTER"; then
  echo "✗ the client is not what was expected afterwards:" >&2
  jq 'del(.credentials)' <<<"$AFTER" >&2
  exit 1
fi
echo "✓ client $NAME ($CLIENT) written and read back: callback, PKCE, confidential, owner group only"
echo "OIDC_CLIENT_ID=$CLIENT"
