#!/usr/bin/env bash
# Usage: run.sh <profile> <tag>   — fresh environment, five tasks, transcripts, grade.
set -u
PROFILE=$1; TAG=$2
cd ~/svrctlrs-eval
export SVRCTLRS_TAG=$TAG EVAL_ADMIN_PASSWORD=$(openssl rand -hex 12) EVAL_ENCRYPTION_KEY=$(openssl rand -hex 32) \
       EVAL_SSH_PASSWORD=$(openssl rand -hex 10)
rm -f old_key old_key.pub && ssh-keygen -q -t ed25519 -N '' -C eval-key-old -f old_key
export EVAL_OLD_PUBKEY="$(cat old_key.pub)" EVAL_OLD_KEY_FILE=$PWD/old_key
docker compose -p svrctlrs-eval -f compose.yml down -v >/dev/null 2>&1
docker compose -p svrctlrs-eval -f compose.yml up -d --quiet-pull >/dev/null 2>&1 || { echo "compose up failed"; exit 1; }
sleep 8
mkdir -p bin out/$PROFILE && docker cp svrctlrs-eval-svrctlrs-1:/app/svrctl bin/svrctl >/dev/null
export SVRCTLRS_TOKEN=$(python3 seed.py) SVRCTLRS_URL=http://127.0.0.1:18990 PATH=$PWD/bin:$PATH
[ -n "$SVRCTLRS_TOKEN" ] || { echo "seed failed"; exit 1; }
for n in 1 2 3 4 5; do
  echo "== task $n ($PROFILE)"
  timeout 1200 hermes -p "$PROFILE" chat --query-file tasks/$n.txt --oneshot --format stream-json \
    --source tool -t terminal,file,skills,todo -s svrctlrs --max-turns 60 --run-budget 900 \
    > out/$PROFILE/task$n.jsonl 2> out/$PROFILE/task$n.err
  echo "exit $?"
done
python3 grade.py | tee out/$PROFILE/grade.txt
