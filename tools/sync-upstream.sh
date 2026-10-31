#!/bin/sh
set -eu
cd "$(git rev-parse --show-toplevel)"
export PATH="$HOME/.cargo/bin:$PATH"
today=$(date +%F)
git fetch upstream --tags --prune
git checkout -q main
git merge --ff-only upstream/main
git push -q origin main
git checkout -q ir
git tag -f "ir-pre-sync-$today"
before=$(git rev-parse --short HEAD)
if ! git rebase upstream/main; then
  echo "sync: rebase stopped on conflicts; resolve, then: git rebase --continue && tools/sync-upstream.sh --finish" >&2
  exit 2
fi
CARGO_TARGET_DIR=target/ir-agent cargo xtask ci --quick
after=$(git rev-parse --short HEAD)
up=$(git describe --tags --always upstream/main)
printf '| %s | %s | %s -> %s | clean | |\n' "$today" "$up" "$before" "$after" >> SYNC-LOG.md
git add SYNC-LOG.md && git commit -q -m "sync: rebase ir on upstream $up ($today)"
git push --force-with-lease origin ir
git push -f origin "ir-pre-sync-$today"
echo "sync: ir rebased on $up, pre-sync tag ir-pre-sync-$today"
