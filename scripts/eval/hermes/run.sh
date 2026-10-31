#!/usr/bin/env bash
# Usage: run.sh <eval profile> <tag>   — fresh environment, every tasks/N.txt, transcripts, grade.
# The profile must be an eval-only one (README, "Eval profiles"): Hermes 0.22 runs every chat in
# its gateway, which never passes this shell's SVRCTLRS_TOKEN on; the token reaches the session
# only through the profile's .env and terminal.env_passthrough. Without it an agent goes looking
# for a token and finds production's (2026-10-31).
set -u
PROFILE=$1; TAG=$2
case $PROFILE in eval-*) ;; *) echo "refused: '$PROFILE' is not an eval-only profile (eval-*)"; exit 2 ;; esac
hermes -p "$PROFILE" config get terminal.env_passthrough 2>/dev/null | grep -q SVRCTLRS_TOKEN \
  || { echo "refused: $PROFILE does not pass SVRCTLRS_TOKEN to its terminal (terminal.env_passthrough)"; exit 2; }
ENV_FILE=$(hermes -p "$PROFILE" config env-path 2>/dev/null | tail -1)
[ -f "$ENV_FILE" ] || { echo "refused: no .env for $PROFILE"; exit 2; }
cd ~/svrctlrs-eval
export SVRCTLRS_TAG=$TAG EVAL_ADMIN_PASSWORD=$(openssl rand -hex 12) EVAL_ENCRYPTION_KEY=$(openssl rand -hex 32) \
       EVAL_SSH_PASSWORD=$(openssl rand -hex 10)
rm -f old_key old_key.pub && ssh-keygen -q -t ed25519 -N '' -C eval-key-old -f old_key
export EVAL_OLD_PUBKEY="$(cat old_key.pub)" EVAL_OLD_KEY_FILE=$PWD/old_key
docker compose -p svrctlrs-eval -f compose.yml down -v >/dev/null 2>&1
docker compose -p svrctlrs-eval -f compose.yml up -d --quiet-pull >/dev/null 2>&1 || { echo "compose up failed"; exit 1; }
sleep 8
# the image's own svrctl, in eval-bin/: bin/svrctl is the one the installed skills name for the
# real deployment (kept current by scripts/deploy/hermes-agent-sync.sh)
mkdir -p eval-bin out/$PROFILE && docker cp svrctlrs-eval-svrctlrs-1:/app/svrctl eval-bin/svrctl >/dev/null
export SVRCTLRS_TOKEN=$(python3 seed.py) SVRCTLRS_URL=http://127.0.0.1:18990 PATH=$PWD/eval-bin:$PATH
[ -n "$SVRCTLRS_TOKEN" ] || { echo "seed failed"; exit 1; }
# this run's URL and token in the eval profile's .env (replacing the last run's)
{ grep -v -E '^(SVRCTLRS_URL|SVRCTLRS_TOKEN)=' "$ENV_FILE"
  echo "SVRCTLRS_URL=$SVRCTLRS_URL"; echo "SVRCTLRS_TOKEN=$SVRCTLRS_TOKEN"; } > "$ENV_FILE.new" \
  && chmod 600 "$ENV_FILE.new" && mv "$ENV_FILE.new" "$ENV_FILE"
# preflight: the session's terminal sees the eval server and this svrctl, or no task runs
timeout 300 hermes -p "$PROFILE" chat --oneshot --format stream-json --source tool -t terminal --max-turns 3 \
  -q 'Run this one shell command and reply with its output only: echo "url=$SVRCTLRS_URL"; command -v svrctl; svrctl status' \
  > out/$PROFILE/preflight.jsonl 2>&1
grep -q 'url=http://127.0.0.1:18990' out/$PROFILE/preflight.jsonl && grep -q "$PWD/eval-bin/svrctl" out/$PROFILE/preflight.jsonl \
  && ! grep -q -E 'NO_TOKEN|TOKEN_INVALID' out/$PROFILE/preflight.jsonl \
  || { echo "preflight failed: the session does not see the eval server (out/$PROFILE/preflight.jsonl)"; exit 1; }
for f in tasks/*.txt; do
  n=$(basename "$f" .txt)
  echo "== task $n ($PROFILE)"
  # no --run-budget: the gateway refuses it; timeout caps the task
  timeout 1200 hermes -p "$PROFILE" chat --query-file "$f" --oneshot --format stream-json \
    --source tool -t terminal,file,skills,todo -s svrctlrs --max-turns 60 \
    > out/$PROFILE/task$n.jsonl 2> out/$PROFILE/task$n.err
  echo "exit $?"
done
python3 grade.py "$PROFILE" | tee out/$PROFILE/grade.txt
