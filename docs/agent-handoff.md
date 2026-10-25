# 「吉大电子资料库」项目交接说明

更新时间：2026-10-25  
项目目录：/Users/lyb/Documents/ChatGPT/项目/college-materials

## 1. 项目目标

这是基于 XMUHub fork 的课程资料共享网站，服务吉林大学电子科学与工程学院学生。

主要功能：邮箱验证码注册和登录、资料上传、审核、分类、搜索、预览、下载、用户反馈、审核员管理和管理员管理。网站的文件存储已迁移到腾讯云 COS，服务器本地资料目录作为保留的回滚和备份副本。

仓库信息：

- 上游：vintcessun/XMUHub
- GitHub fork：littlepapa-alt/XMUHub
- 当前分支：codex/college-site-setup

## 2. 站点信息

站点配置：site/my-college/site.json

- 名称：吉大电子资料库
- 学校：吉林大学电子科学与工程学院
- 当前访问地址：http://49.232.142.58
- domain 配置：49.232.142.58
- 管理员邮箱：15338013270@163.com
- 校园邮箱认证域名：jlu.edu.cn
- GitHub 仓库：https://github.com/littlepapa-alt/XMUHub

站点资源目录：site/my-college/assets/site/

- logo.png：学院院徽，全站 logo
- university-logo.jpg：吉林大学校徽
- background.jpg：网站背景图
- theme.css、learning.css：主题和学习页面样式
- browse.js、catalog.js、home.js：目录和首页逻辑

已完成的界面调整：

- 删除首页 GitHub 推广卡片和“去 GitHub 看看”入口；
- 删除顶部“开源”入口；
- 删除教程入口；
- 删除底部教程、使用须知、意见反馈那一行；
- 删除底部源代码链接的显示；
- 教程、审核、提醒等界面统一到课程目录色调；
- 设置学院院徽、吉林大学校徽和新的背景图；
- 部署网页前会显示“网站正在更新，请稍后再来”的维护公告。

site.json 中仍保留 repo 字段，因为项目采用 AGPL-3.0。页面是否显示链接和是否保留源码配置是两件事，后续不要随意删掉 repo 字段。

## 3. 课程和资料

目前重点课程为“大三上学期”，包括：

- 高频电子技术
- 微波技术
- 通信原理
- 数字信号处理
- 单片机原理

资料曾按课件合集、其他资料合集等方式整理，目标是把同一课程的章节和零散材料集中展示。

课程、分类、资源元数据、审核状态和用户信息在服务器数据库中，不在 GitHub 仓库中。上传的资料文件也不能提交进 GitHub 仓库。

资料原则：

- 审核、下架和删除申请不直接删除底层文件；
- 修改分类优先使用管理员页面或正式 API；
- 批量整理前先备份服务器数据；
- 不要把服务器数据库或资料文件提交到仓库。

## 4. 服务器

- 腾讯云 IP：49.232.142.58
- Ubuntu 22.04
- 已安装 Docker、1Panel、Nginx
- 程序目录：/opt/xmuhub/
- 数据目录：/var/lib/xmuhub/
- systemd 服务：xmuhub.service
- Nginx 反代：127.0.0.1:8089
- 维护脚本：/opt/xmuhub/maintenance.py

数据库是 redb 嵌入式数据库，账号、分类、资源元数据和审核状态在服务器数据目录中。

已检查到服务器部署标记：

~~~text
backend=manual COS signature-fix build (线上二进制)
web=80f6e52
~~~

COS 后端已部署到线上，服务器环境中的 `XMUHUB_STORAGE=cos` 已生效。签名实现已按腾讯云 SDK 的规则修复，并在服务器上重新编译部署。

## 5. COS 改造

de30c68 已加入：

- 腾讯云 COS 私有存储后端；
- COS 临时上传和下载链接；
- 浏览器直接上传和下载，网站服务器不转发资料字节；
- 管理员 COS 迁移接口；
- scripts/migrate-cos.py 迁移脚本；
- GitHub 外部源迁移；
- 服务器本地资料先复制到电脑，再上传 COS 的 local-dir 模式；
- 多分片迁移、SHA-256 校验和断点续传；
- COS 配置模板和部署说明。

114de21 修复了 COS 签名的 RFC 3986 编码：主机名和元数据值中的 `-`、`.`、`_`、`~` 会按 COS 要求保留。对应的 COS 单元测试已通过。

主要代码：

- crates/xmuhub-core/src/storage/cos.rs
- crates/xmuhub-core/src/storage/mod.rs
- crates/xmuhub-server/src/config.rs
- crates/xmuhub-server/src/main.rs
- crates/xmuhub-server/src/api.rs
- scripts/migrate-cos.py
- .secrets.example/cos.env.example

本地 COS 配置：.secrets/cos.env

该文件已被 Git 忽略，不能输出、复制、提交或发送其中的 SecretId、SecretKey。可以只检查字段是否存在、长度和格式。当前本地配置已经填写并设置为 600 权限。

配置格式：

~~~env
COS_SECRET_ID=...
COS_SECRET_KEY=...
COS_BUCKET=完整Bucket名称
COS_REGION=ap-beijing
COS_ENDPOINT=
COS_SIGNED_URL_TTL=1800
COS_UPLOAD_URL_TTL=3600
~~~

COS Bucket 要求：

- 北京地域；
- 私有 Bucket；
- CORS 允许 Origin：http://49.232.142.58；
- CORS Methods：GET、HEAD、PUT、OPTIONS；
- Allowed Headers：*；
- Expose Headers：ETag、Content-Length、x-cos-meta-sha256。

## 6. COS 部署和迁移

完整后端部署需要本地存在：

- .secrets/deploy.env
- .secrets/upload.env
- .secrets/cos.env
- 必要的邮箱和管理员配置

当前这台电脑缺少 deploy.env 和 upload.env，因此不能直接运行完整的 scripts/deploy.ps1。scripts/deploy-web.sh 只部署网页和站点资源，不能部署 COS 后端。启用 COS 必须重新编译并部署 Rust 后端程序。本次已在服务器 Docker 编译环境中完成低内存 release 编译并部署。

标准流程：

~~~powershell
pwsh scripts/build-alinux3.ps1
pwsh scripts/deploy.ps1
~~~

部署前先显示公告：

~~~sh
ssh -i /安全位置/macbookpro.pem root@49.232.142.58 \
  'python3 /opt/xmuhub/maintenance.py start'
~~~

部署、健康检查和人工验证完成后撤下公告：

~~~sh
ssh -i /安全位置/macbookpro.pem root@49.232.142.58 \
  'python3 /opt/xmuhub/maintenance.py finish'
~~~

线上 COS 已确认：

- `xmuhub.service` 正常运行；
- `XMUHUB_STORAGE=cos` 和 COS 配置已被服务进程读取；
- 管理迁移接口返回 `primary: cos`；
- `/var/lib/xmuhub/data/files` 中保留 427 个本地分片、约 2.42 GB；
- 数据库中有 348 个资料记录、427 个分片，迁移接口待迁移数量为 0；
- COS 私有 Bucket 中有 427 个 `blobs/` 对象，总大小为 2,419,295,009 字节；
- `/api/meta` 的 `stored_bytes` 与 COS 对象总大小一致；
- 已保存数据库备份：`/opt/xmuhub/releases/xmuhub-redb-before-cos-migration-20261025105518`；
- 原始本地文件目录保留，后续完成独立备份前不要删除。

迁移已使用服务器本地资料作为源完成。迁移断点文件为 `.secrets/cos-migration-state.json`；服务器端仍保留本地资料，未删除任何原文件。后续重复核对可运行：

~~~sh
python3 scripts/migrate-cos.py \
  --base-url http://49.232.142.58 \
  --dry-run
~~~

该命令应返回 0 个待迁移文件。迁移脚本需要 `.secrets/upload.env` 中的 `XMUHUB_SCRIPT_TOKEN`，不要把令牌提交到仓库。

已通过腾讯云 SDK 生成私有对象的 5 分钟临时下载链接并实际读取：HTTP 200，下载大小与对象大小一致，SHA-256 与对象 key 和 `x-cos-meta-sha256` 一致。应用统计和迁移接口验证了数据库中的 COS 副本已经挂载；浏览器端下载仍需使用已登录账号在页面点击验证，匿名访问会被登录保护拦截。

## 7. 邮箱和注册

- 管理员邮箱：15338013270@163.com；
- QQ 邮箱曾作为主发信邮箱；
- 网易邮箱作为备用发信邮箱；
- QQ 曾出现 550 The mail may contain inappropriate words or content；
- 网易邮箱已成功收到测试验证码；
- 校园邮箱已验证可以注册；
- 已加入空值、格式校验和收件人拒收提示；
- 主 SMTP 发送失败时尝试备用 SMTP，两个邮箱使用同一个验证码。

不要承诺网站能在发送前判断邮箱账号是否真实存在。很多邮箱服务商只能在 SMTP 投递时退信，网站只能做格式校验、空值校验、错误提示和备用 SMTP 重试。

邮箱密钥只放在：

- .secrets/mail.env
- .secrets/mail-backup.env
- .secrets/mail-qq-update.env

授权码不能提交到 GitHub，也不能发到聊天。

## 8. SSH 和密钥

macbookpro.pem 是腾讯云 SSH 私钥：

- 权限设为 600；
- 不要提交到 GitHub；
- 不要复制给其他人；
- 不要在终端输出私钥内容；
- 换电脑时重新配置 SSH 权限。

以下内容只能放在本地 .secrets/：

- COS SecretId、SecretKey；
- SMTP 授权码；
- SSH 部署配置；
- 上传签名密钥；
- XMUHUB_SCRIPT_TOKEN；
- GitHub Token；
- Cloudflare Token。

## 9. 修改边界

默认优先修改：

- site/my-college/
- 必要的项目文档；
- 必要的配置模板。

公共代码目录：

- web/
- crates/
- scripts/
- worker/

这些目录和上游共用，改动会增加合并冲突。COS 功能已经修改了公共后端代码，因此后续如果继续修改 COS，必须保持现有存储接口、权限校验、分片校验和迁移流程，不要另起一套上传逻辑。

开始工作先执行：

~~~sh
git status --short --branch
git log -5 --oneline
~~~

不要误提交当前未跟踪内容：

- .DS_Store
- 网站建设/（需要先确认用途）

## 10. 测试和线上检查

后端和 COS 改动至少运行：

~~~sh
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
python3 -m py_compile scripts/migrate-cos.py
git diff --check
~~~

前端检查：

~~~sh
node scripts/check-web.mjs
node scripts/check-rules.mjs
~~~

此前电脑没有 node，上面两个 Node 检查尚未执行；如果工作区依赖提供 Node，再补跑这两项。

线上检查：

~~~sh
systemctl is-active xmuhub.service
curl -I http://49.232.142.58/
curl -sS http://127.0.0.1:8089/ >/dev/null
~~~

人工验证：

1. 获取注册验证码；
2. 验证 QQ 失败时是否切换网易；
3. 注册和登录；
4. 上传小文件；
5. 管理员审核；
6. 检查课程分类和合集；
7. 下载临时链接；
8. 检查 COS 直传和直下载；
9. 核对迁移后的数量和摘要；
10. 检查维护公告显示和撤下。

## 11. 当前待办

1. 使用已登录账号在网页端实测一次资料下载，确认浏览器按应用下载计划直连 COS；
2. 确认 COS 私有 Bucket 的 CORS 覆盖当前站点 Origin、GET、HEAD、PUT、OPTIONS 和所需响应头；
3. 实测上传、审核、下载全链路；
4. 建立 `/var/lib/xmuhub/` 和 `/opt/xmuhub/` 的定期备份，必要时异地备份到 COS；
5. 配置域名、HTTPS、备案和 CDN；
6. 将手工部署恢复成可重复的标准脚本流程；
7. 维护公告仍在首页显示，完成登录态下载和发布前检查后运行 `python3 /opt/xmuhub/maintenance.py finish`。

## 12. 给其他智能体的直接提示词

你正在维护一个基于 XMUHub 的吉林大学电子科学与工程学院资料共享网站。请先阅读本文件、AGENTS.md、docs/development.md 和 docs/principles.md，再查看 git status。

项目路径是 /Users/lyb/Documents/ChatGPT/项目/college-materials。当前分支是 codex/college-site-setup。服务器是 49.232.142.58，程序在 /opt/xmuhub/，数据在 /var/lib/xmuhub/，服务名是 xmuhub.service。

不要读取、输出、提交或发送 .secrets/ 中的任何密钥，特别是 COS、SMTP、SSH 和脚本令牌。不要直接修改服务器文件，先在 GitHub 分支修改、测试、提交，再按部署流程发布。线上更新前先显示维护公告。

COS 功能在提交 de30c68 中实现，签名修复在 114de21 之后继续补充了时间窗口和 HMAC 派生修复，线上后端已部署对应版本。不要把 scripts/deploy-web.sh 当成后端部署脚本；COS 后端必须通过完整 Rust 编译和标准部署流程发布。继续工作时保持本地资料副本，不能在未完成备份前删除。完成后说明修改内容、测试命令、线上验证结果和剩余风险。

