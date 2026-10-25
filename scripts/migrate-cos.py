#!/usr/bin/env python3
"""Migrate existing material replicas to COS from the user's computer.

The XMUHub server is used only for JSON metadata, signed upload URLs and confirmation.
Material bytes go directly from a returned source URL to this computer, then directly
from this computer to COS.

Example:
  python3 scripts/migrate-cos.py --base-url http://49.232.142.58

The script reads XMUHUB_SCRIPT_TOKEN from .secrets/upload.env unless the variable is
already present in the environment. It never prints the token.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request


ROOT = pathlib.Path(__file__).resolve().parents[1]


def load_env(path: pathlib.Path) -> dict[str, str]:
    values: dict[str, str] = {}
    if not path.exists():
        return values
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        values[key.strip()] = value.strip().strip('"').strip("'")
    return values


def request_json(url: str, token: str, method: str = "GET", body: dict | None = None) -> dict:
    data = None if body is None else json.dumps(body, ensure_ascii=False).encode("utf-8")
    headers = {"Authorization": f"Bearer {token}", "Accept": "application/json"}
    if data is not None:
        headers["Content-Type"] = "application/json"
    req = urllib.request.Request(url, data=data, headers=headers, method=method)
    try:
        with urllib.request.urlopen(req, timeout=60) as response:
            return json.loads(response.read().decode("utf-8"))
    except urllib.error.HTTPError as exc:
        detail = exc.read().decode("utf-8", errors="replace")[:500]
        raise RuntimeError(f"网站接口失败 HTTP {exc.code}: {detail}") from exc


def download_source(urls: list[str], destination: pathlib.Path, expected_size: int, expected_sha256: str) -> str:
    last_error: Exception | None = None
    for url in urls:
        try:
            digest = hashlib.sha256()
            size = 0
            req = urllib.request.Request(url, headers={"User-Agent": "XMUHub-COS-migration/1.0"})
            with urllib.request.urlopen(req, timeout=120) as response, destination.open("wb") as out:
                while True:
                    chunk = response.read(1024 * 1024)
                    if not chunk:
                        break
                    out.write(chunk)
                    digest.update(chunk)
                    size += len(chunk)
            actual = digest.hexdigest()
            if size != expected_size or actual != expected_sha256:
                raise RuntimeError(f"源文件校验失败：大小 {size}/{expected_size}，sha256 {actual}/{expected_sha256}")
            return url
        except (OSError, urllib.error.URLError, RuntimeError) as exc:
            last_error = exc
            destination.unlink(missing_ok=True)
    raise RuntimeError(f"所有源地址都无法通过校验：{last_error}")


def copy_local_source(source: pathlib.Path, destination: pathlib.Path, expected_size: int, expected_sha256: str) -> str:
    digest = hashlib.sha256()
    size = 0
    try:
        with source.open("rb") as src, destination.open("wb") as out:
            while True:
                chunk = src.read(1024 * 1024)
                if not chunk:
                    break
                out.write(chunk)
                digest.update(chunk)
                size += len(chunk)
    except OSError as exc:
        raise RuntimeError(f"本地资料无法读取：{source}") from exc
    actual = digest.hexdigest()
    if size != expected_size or actual != expected_sha256:
        destination.unlink(missing_ok=True)
        raise RuntimeError(f"本地文件校验失败：大小 {size}/{expected_size}，sha256 {actual}/{expected_sha256}")
    return str(source)


def upload_to_cos(url: str, headers: list[list[str]], path: pathlib.Path, size: int) -> None:
    request_headers = {name: value for name, value in headers}
    request_headers["Content-Length"] = str(size)
    try:
        with path.open("rb") as body:
            req = urllib.request.Request(url, data=body, headers=request_headers, method="PUT")
            with urllib.request.urlopen(req, timeout=300) as response:
                response.read(0)
    except urllib.error.HTTPError as exc:
        detail = exc.read().decode("utf-8", errors="replace")[:300]
        raise RuntimeError(f"COS 直传失败 HTTP {exc.code}: {detail}") from exc


def main() -> int:
    parser = argparse.ArgumentParser(description="从已有资料源直接迁移资料到腾讯云 COS")
    parser.add_argument("--base-url", required=True, help="网站地址，例如 http://49.232.142.58")
    parser.add_argument("--batch", type=int, default=20, help="每批获取的分片数，默认 20")
    parser.add_argument("--max-bytes", type=int, default=5 * 1024 * 1024 * 1024, help="每批最多迁移字节数")
    parser.add_argument("--state", default=str(ROOT / ".secrets" / "cos-migration-state.json"), help="断点文件路径")
    parser.add_argument("--local-dir", help="服务器本地资料目录的电脑副本，例如 ./xmuhub-files；不填写则跳过没有外部源的资料")
    parser.add_argument("--dry-run", action="store_true", help="只列出候选，不下载或上传")
    args = parser.parse_args()

    env = load_env(ROOT / ".secrets" / "upload.env")
    token = os.environ.get("XMUHUB_SCRIPT_TOKEN") or env.get("XMUHUB_SCRIPT_TOKEN", "")
    if len(token) < 32:
        raise SystemExit("缺少有效的 XMUHUB_SCRIPT_TOKEN，请填写 .secrets/upload.env")
    base = args.base_url.rstrip("/")
    local_dir = pathlib.Path(args.local_dir).expanduser() if args.local_dir else None
    state_path = pathlib.Path(args.state)
    state = json.loads(state_path.read_text(encoding="utf-8")) if state_path.exists() else {}
    done = set(state.get("done", []))

    query = urllib.parse.urlencode({"limit": args.batch, "max_bytes": args.max_bytes})
    listing = request_json(f"{base}/api/admin/storage/cos/migration?{query}", token)
    items = listing.get("items", [])
    print(f"本批候选：{len(items)} 个文件，约 {listing.get('bytes', 0)} 字节")
    if args.dry_run:
        for item in items:
            print(f"  {item['key']} ({item['size']} bytes, {len(item['parts'])} 个分片)")
        return 0

    migrated = 0
    skipped = 0
    for item in items:
        for part in item["parts"]:
            part_id = f"{item['key']}:{part['index']}:{part['sha256']}"
            if part_id in done:
                skipped += 1
                continue
            sources = part.get("sources", [])
            local_name = part.get("local_name")
            local_path = None
            if local_dir and isinstance(local_name, str) and pathlib.Path(local_name).name == local_name:
                local_path = local_dir / local_name
            if not sources and local_path is None:
                print(f"跳过（没有可用的外部源地址）：{part_id}")
                continue
            with tempfile.TemporaryDirectory(prefix="xmuhub-cos-") as temp:
                path = pathlib.Path(temp) / "part"
                source = (
                    download_source(sources, path, part["size"], part["sha256"])
                    if sources
                    else copy_local_source(local_path, path, part["size"], part["sha256"])
                )
                prepared = request_json(
                    f"{base}/api/admin/storage/cos/migration",
                    token,
                    "POST",
                    {"key": item["key"], "index": part["index"], "size": part["size"], "sha256": part["sha256"]},
                )
                if prepared.get("already"):
                    done.add(part_id)
                    skipped += 1
                    continue
                target = prepared["target"]
                for attempt in range(3):
                    try:
                        upload_to_cos(target["url"], target.get("headers", []), path, part["size"])
                        break
                    except RuntimeError:
                        if attempt == 2:
                            raise
                        time.sleep(2 ** attempt)
                request_json(
                    f"{base}/api/admin/storage/cos/migration/confirm",
                    token,
                    "POST",
                    {"key": item["key"], "index": part["index"], "size": part["size"], "sha256": part["sha256"]},
                )
                done.add(part_id)
                migrated += 1
                state_path.parent.mkdir(parents=True, exist_ok=True)
                state_path.write_text(json.dumps({"done": sorted(done)}, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
                print(f"已迁移 {migrated}：{item['key']} 分片 {part['index']}（源：{source}）")
    state_path.parent.mkdir(parents=True, exist_ok=True)
    state_path.write_text(json.dumps({"done": sorted(done)}, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"完成：新迁移 {migrated}，断点跳过 {skipped}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except KeyboardInterrupt:
        raise SystemExit("已中断；再次运行会从断点继续")
