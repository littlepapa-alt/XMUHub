# 密钥和部署配置模板

把这个目录复制成 `.secrets/`（已被 git 忽略，永远不要提交），去掉每个文件名末尾的 `.example`，再按注释填写：

```
cp -r .secrets.example .secrets
cd .secrets && for f in *.example; do mv "$f" "${f%.example}"; done
```

| 文件 | 用途 | 必需 |
|---|---|---|
| cos.env | 腾讯云 COS 私有 Bucket 和 API 密钥 | 使用 COS 存储时 |
| `deploy.env` | 部署到哪台服务器、网站地址、用哪个 `site/` 文件夹、Cloudflare 区域 | 是 |
| `github.env` | GitHub 账号和令牌；COS 模式下仅在保留旧 GitHub 资料或使用 GitHub 导入/备份时需要 | COS 模式可选 |
| `upload.env` | 上传凭证的签名密钥、上传 Worker 名称、脚本令牌 | 是 |
| `cloudflare.env` | Cloudflare 账号、API 令牌、上传 Worker 域名、Turnstile 人机验证 | 用 Cloudflare 时 |
| `mail.env` | 发验证码邮件的 SMTP | 要邮箱注册时 |
| `admins.txt` | 管理员邮箱，一行一个 | 是 |
| `ddos-allow.txt` | 源站防护的放行地址（见 deploy/flood-guard/） | 可选 |

所有密钥都只在部署时写进服务器上的环境文件，不会编进程序，也不会进入仓库；换密钥只要改这里再部署一次。

启用 COS 后，先把 `.secrets/cos.env` 填好并部署，使网站主存储变为 COS。已有资料迁移使用本地脚本：

```
python3 scripts/migrate-cos.py --base-url http://49.232.142.58 --dry-run
python3 scripts/migrate-cos.py --base-url http://49.232.142.58
# 如果旧资料只有服务器本地副本，先用 scp 把 /var/lib/xmuhub/files 复制到电脑，再指定副本目录：
python3 scripts/migrate-cos.py --base-url http://49.232.142.58 --local-dir ./xmuhub-files
```

脚本只通过网站获取清单和 COS 临时上传地址，资料字节由你的电脑从已有外部源下载，或从你用 `scp` 准备好的本地副本读取，然后直接上传 COS；它不会通过网站下载本地文件。断点保存在 `.secrets/cos-migration-state.json`，该文件不入库。
