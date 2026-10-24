# 网站更新公告

每次修改线上网页、后端或服务配置前，先在首页显示：

> 网站正在更新，请稍后再来。感谢理解！

服务器上已安装 `/opt/xmuhub/maintenance.py`，通过 Nginx 在页面顶部显示公告，不需要后台登录。首次安装会备份原来的 Nginx 配置，检查配置语法并确认首页包含公告。原有站点公告不受影响。

```sh
python3 /opt/xmuhub/maintenance.py start
# 完成部署或配置更新，并检查服务、首页及本次涉及的功能。
python3 /opt/xmuhub/maintenance.py finish
python3 /opt/xmuhub/maintenance.py status
```

`scripts/deploy-web.sh` 默认部署本站时会自动发布、撤下公告；其他部署方式必须先执行 `start`。失败后先确认网站是否已经恢复；未恢复时保留公告。公告不阻止访问，也不会让正在浏览旧页面的用户自动刷新。

此工具适用于当前腾讯云服务器的 Nginx 配置：`/etc/nginx/sites-available/xmuhub`，反向代理至 `127.0.0.1:8089`。更换服务器或代理配置时须同步调整并检查。
