#!/usr/bin/env bash
# 部署网页到腾讯云服务器：打包 web/ + site/my-college（作为 web/_site/），
# 上传后原子切换 /opt/xmuhub/web 并重启 xmuhub.service；失败自动回滚。
# 用法：scripts/deploy-web.sh [-n|--dry-run]
# 可用环境变量覆盖：SSH_HOST SSH_USER SSH_KEY REMOTE_BASE SITE_DIR SERVICE
set -euo pipefail

SSH_HOST="${SSH_HOST:-49.232.142.58}"
SSH_USER="${SSH_USER:-root}"
SSH_KEY="${SSH_KEY:-$HOME/.ssh/macbookpro.pem}"
REMOTE_BASE="${REMOTE_BASE:-/opt/xmuhub}"
SITE_DIR="${SITE_DIR:-site/my-college}"
SERVICE="${SERVICE:-xmuhub.service}"
DRY_RUN=0
[ "${1:-}" = "-n" ] || [ "${1:-}" = "--dry-run" ] && DRY_RUN=1

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

[ -f "$SSH_KEY" ] || { echo "找不到 SSH 私钥：$SSH_KEY" >&2; exit 1; }
[ -f "$SITE_DIR/site.json" ] || { echo "找不到站点目录：$SITE_DIR" >&2; exit 1; }

SSH=(ssh -o BatchMode=yes -o ConnectTimeout=10 -i "$SSH_KEY" "$SSH_USER@$SSH_HOST")
SCP=(scp -o BatchMode=yes -i "$SSH_KEY")

REV="$(git rev-parse --short HEAD)"
DIRTY=""; [ -n "$(git status --porcelain -- web "$SITE_DIR")" ] && DIRTY="-dirty"
echo "部署版本：$REV$DIRTY → $SSH_USER@$SSH_HOST:$REMOTE_BASE/web"

STAGE="$(mktemp -d)"; trap 'rm -rf "$STAGE"' EXIT
mkdir -p "$STAGE/_site"
cp -R "$SITE_DIR/" "$STAGE/_site/"
COPYFILE_DISABLE=1 tar --no-xattrs --no-mac-metadata -czf "$STAGE/web.tar.gz" -C web . -C "$STAGE" _site
echo "打包完成：$(du -h "$STAGE/web.tar.gz" | cut -f1)"

if [ "$DRY_RUN" = 1 ]; then echo "（dry-run）不上传、不重启"; exit 0; fi

"${SCP[@]}" "$STAGE/web.tar.gz" "$SSH_USER@$SSH_HOST:$REMOTE_BASE/web.tar.gz"

"${SSH[@]}" "bash -s" <<REMOTE
set -e
cd '$REMOTE_BASE'
rm -rf web.new && mkdir web.new
tar -xzf web.tar.gz -C web.new && rm -f web.tar.gz
find web.new -name '._*' -delete
chown -R xmuhub:xmuhub web.new
rm -rf web.old
[ -d web ] && mv web web.old
mv web.new web
printf 'web=%s%s\n' '$REV' '$DIRTY' > DEPLOYED
systemctl restart '$SERVICE'
ok=0
for i in 1 2 3 4 5 6; do
  sleep 2
  if systemctl is-active --quiet '$SERVICE' && curl -sf -o /dev/null --max-time 3 http://127.0.0.1:8089/; then ok=1; break; fi
done
if [ "\$ok" = 1 ]; then
  echo "服务已重启，健康检查通过"
  rm -rf web.old
else
  echo "服务启动失败，回滚到旧版本" >&2
  journalctl -u '$SERVICE' --no-pager -n 10 >&2 || true
  mv web web.bad && mv web.old web
  systemctl restart '$SERVICE' || true
  exit 1
fi
REMOTE
echo "部署完成：http://$SSH_HOST"
