#!/usr/bin/env bash
# 备份服务器数据到本地：/var/lib/xmuhub（数据库+资料文件）和 /opt/xmuhub（网页+配置）。
# 为保证数据库一致性，备份期间短暂停止服务（通常几秒），完成后自动恢复。
# 用法：scripts/backup-server.sh [--live]   （--live 不停机，数据库可能不一致，仅应急）
set -euo pipefail

SSH_HOST="${SSH_HOST:-49.232.142.58}"
SSH_USER="${SSH_USER:-root}"
SSH_KEY="${SSH_KEY:-$HOME/.ssh/macbookpro.pem}"
SERVICE="${SERVICE:-xmuhub.service}"
BACKUP_DIR="${BACKUP_DIR:-$HOME/backups/xmuhub}"
LIVE=0
[ "${1:-}" = "--live" ] && LIVE=1

TS="$(date +%Y%m%d-%H%M%S)"
OUT="$BACKUP_DIR/xmuhub-$TS.tar.gz"
mkdir -p "$BACKUP_DIR"

SSH=(ssh -o BatchMode=yes -o ConnectTimeout=10 -i "$SSH_KEY" "$SSH_USER@$SSH_HOST")

echo "备份 $SSH_USER@$SSH_HOST → $OUT"
if [ "$LIVE" = 1 ]; then
  "${SSH[@]}" "tar -czf - -C / var/lib/xmuhub opt/xmuhub 2>/dev/null" > "$OUT"
else
  "${SSH[@]}" "
    set -e
    trap 'systemctl start $SERVICE' EXIT
    systemctl stop $SERVICE
    tar -czf - -C / var/lib/xmuhub opt/xmuhub
  " > "$OUT"
fi
echo "完成：$OUT ($(du -h "$OUT" | cut -f1))"
# 只保留最近 10 份
ls -t "$BACKUP_DIR"/xmuhub-*.tar.gz 2>/dev/null | tail -n +11 | xargs rm -f 2>/dev/null || true
