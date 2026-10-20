# 团队协作流程

本仓库使用两个远程仓库：

- `origin`：`littlepapa-alt/XMUHub`，存放学院站点的开发分支和最终代码。
- `upstream`：`vintcessun/XMUHub`，用于同步上游项目更新。

## 开发功能

从最新主分支创建功能分支：

```bash
git switch main
git pull --ff-only origin main
git switch -c feature/功能名称
```

开发完成后检查并提交：

```bash
cargo check --workspace
git status
git add <修改的文件>
git commit -m "说明本次修改"
git push -u origin feature/功能名称
```

在 GitHub 上创建 Pull Request，合并前至少通过编译检查和人工页面测试。

## 同步上游

开始新功能前，先同步原项目：

```bash
git fetch upstream
git switch main
git merge --ff-only upstream/main
git push origin main
```

如果上游更新和本地修改发生冲突，在功能分支上解决冲突并完成测试后再提交。

## 站点配置

学院自己的站点内容放在 `site/my-college/`，包括站名、学院名称、域名和公告。密钥、服务器地址和管理员名单放在本地 `.secrets/`，不要提交到 GitHub。
