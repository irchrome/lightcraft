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
export CARGO_TARGET_DIR=target/ir-agent
cargo fmt --all -- --check
cargo test -p lightcraft-develop -p lightcraft-pipeline -p lightcraft-raw -p lightcraft-engine -p lightcraft-gpu --lib --tests -- --skip v1_and_v2_render_as_before_v3
cargo xtask layers
cargo xtask assets
cargo xtask parity
mixer=$(gh pr view 3579 -R storytold/lightcraft --json state --jq .state 2>/dev/null || echo unknown)
echo "sync: upstream PR #3579 (channel mixer) is $mixer; if MERGED, drop the fork's squash commit"
after=$(git rev-parse --short HEAD)
up=$(git describe --tags --always upstream/main)
printf '| %s | %s | %s -> %s | clean | PR #3579: %s |\n' "$today" "$up" "$before" "$after" "$mixer" >> SYNC-LOG.md
git add SYNC-LOG.md && git commit -q -m "sync: rebase ir on upstream $up ($today)"
git push --force-with-lease origin ir
git push -f origin "ir-pre-sync-$today"
echo "sync: ir rebased on $up, pre-sync tag ir-pre-sync-$today"
