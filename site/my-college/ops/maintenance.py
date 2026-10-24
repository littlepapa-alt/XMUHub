#!/usr/bin/env python3
"""首页更新公告；在服务器以 root 运行，独立于应用重启。"""
import argparse
from pathlib import Path
import subprocess
import time
import urllib.request

CONFIG = Path('/etc/nginx/sites-available/xmuhub')
SNIPPET = Path('/etc/nginx/snippets/xmuhub-maintenance.conf')
MARKER = 'id="site-update-notice"'
NOTICE = '网站正在更新，请稍后再来。感谢理解！'
INCLUDE = '        include /etc/nginx/snippets/xmuhub-maintenance.conf;\n'
BANNER = (
    '<div id="site-update-notice" role="alert" '
    'style="background:#edf2f8;color:#183954;border-bottom:1px solid #cbd7e4;'
    'padding:16px 24px;text-align:center;font-size:16px;line-height:1.6">'
    '<strong>更新公告</strong>　' + NOTICE + '</div>'
)


def page():
    request = urllib.request.Request('http://127.0.0.1/', headers={'Cache-Control': 'no-cache'})
    with urllib.request.urlopen(request, timeout=10) as response:
        return response.read().decode()


def change(enabled):
    config = CONFIG.resolve(strict=True)
    original = config.read_text()
    previous = SNIPPET.read_text() if SNIPPET.exists() else None
    new_config = original
    if INCLUDE not in original:
        anchor = '        proxy_pass http://127.0.0.1:8089;\n'
        if original.count(anchor) != 1:
            raise RuntimeError('Nginx 配置发生变化，请先检查公告插入位置')
        new_config = original.replace(anchor, INCLUDE + anchor)
    if new_config != original:
        backup = Path('/opt/xmuhub/releases') / ('nginx-before-maintenance-' + time.strftime('%Y%m%d-%H%M%S'))
        backup.mkdir(mode=0o700, parents=True)
        (backup / 'xmuhub.conf').write_text(original)
    snippet = (
        'proxy_set_header Accept-Encoding "";\n'
        'proxy_set_header If-None-Match "";\n'
        'proxy_set_header If-Modified-Since "";\n'
        'add_header Cache-Control "no-store" always;\n'
        "sub_filter '</header>' '</header>" + BANNER + "';\n"
        'sub_filter_once on;\n'
    ) if enabled else '# No update in progress.\n'
    SNIPPET.parent.mkdir(parents=True, exist_ok=True)
    try:
        SNIPPET.write_text(snippet)
        config.write_text(new_config)
        subprocess.run(['nginx', '-t'], check=True, capture_output=True)
        subprocess.run(['systemctl', 'reload', 'nginx'], check=True)
        # Nginx reload is asynchronous: wait until a new worker serves the notice.
        for _ in range(20):
            if (MARKER in page()) == enabled:
                print('更新公告已发布' if enabled else '更新公告已撤下')
                return
            time.sleep(0.25)
        raise RuntimeError('首页公告状态未通过检查')
    except Exception:
        config.write_text(original)
        if previous is None:
            SNIPPET.unlink(missing_ok=True)
        else:
            SNIPPET.write_text(previous)
        subprocess.run(['nginx', '-t'], check=True, capture_output=True)
        subprocess.run(['systemctl', 'reload', 'nginx'], check=True)
        raise


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['start', 'finish', 'status'])
    action = parser.parse_args().action
    if action == 'status':
        print('更新中' if MARKER in page() else '正常运行')
    else:
        change(action == 'start')
