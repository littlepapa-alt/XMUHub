#!/usr/bin/env bash
# 把服务器上的 XMUHub 每日备份同步到本机（异地副本）。
# 由本机 launchd 每小时调用；Mac 睡眠错过的时段唤醒后自动补跑。
# 只拉取 xmuhub-*.tar.gz 归档；服务器不可达时静默跳过，不影响本机。
set -u

KEY="$HOME/.ssh/macbookpro.pem"
HOST=root@49.232.142.58
REMOTE=/var/backups/xmuhub
DEST="$HOME/Documents/ChatGPT/项目/college-materials-backups"
LOG="$DEST/sync.log"
KEEP=14

mkdir -p "$DEST"
chmod 700 "$DEST"

# 服务器不可达（Mac 离线/服务器维护）时安静退出
ssh -o BatchMode=yes -o ConnectTimeout=8 -i "$KEY" "$HOST" true 2>/dev/null || exit 0

# 只同步归档文件（--ignore-existing 不覆盖已拉取的）
rsync -a --ignore-existing --include='xmuhub-*.tar.gz' --exclude='*' \
  -e "ssh -o BatchMode=yes -o ConnectTimeout=8 -i $KEY" \
  "$HOST:$REMOTE/" "$DEST/" 2>>"$LOG"

# 本机保留最近 KEEP 份
ls -1t "$DEST"/xmuhub-*.tar.gz 2>/dev/null | tail -n +$((KEEP + 1)) | xargs -r rm -f

echo "$(date '+%F %T') sync ok, local archives: $(ls -1 "$DEST"/xmuhub-*.tar.gz 2>/dev/null | wc -l | tr -d ' ')" >> "$LOG"
