#!/usr/bin/env bash
# XMUHub 每日备份：数据库 + 配置 + 部署标记 → /var/backups/xmuhub/，保留 7 天。
# 由 cron 调用（scripts/install-backup.sh 安装）。资料文件本体在 COS 多副本存储，不在备份范围。
# redb 具备崩溃一致性（WAL + 提交点），凌晨低峰直接复制文件是安全的。
set -euo pipefail

DATA=/var/lib/xmuhub/data
APP=/opt/xmuhub
DEST=/var/backups/xmuhub
KEEP=7
TS=$(date +%Y%m%d-%H%M%S)
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

mkdir -p "$DEST"
chmod 700 "$DEST"

cp -p "$DATA/xmuhub.redb" "$WORK/"
cp -p "$APP/xmuhub.env" "$WORK/"
cp -p "$APP/DEPLOYED" "$WORK/" 2>/dev/null || true

ARCHIVE="$DEST/xmuhub-$TS.tar.gz"
tar -czf "$ARCHIVE" -C "$WORK" .
chmod 600 "$ARCHIVE"

# 完整性校验：归档可解开且数据库文件在内
tar -tzf "$ARCHIVE" | grep -q "xmuhub.redb"

# 只保留最近 KEEP 份（仅匹配本脚本产生的归档）
ls -1t "$DEST"/xmuhub-*.tar.gz 2>/dev/null | tail -n +$((KEEP + 1)) | xargs -r rm -f

SIZE=$(du -h "$ARCHIVE" | cut -f1)
echo "$(date '+%F %T') backup ok: $ARCHIVE ($SIZE), kept $(ls -1 "$DEST"/xmuhub-*.tar.gz | wc -l) archives"
