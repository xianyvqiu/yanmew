"""Export the SDK and compiled validator without exporting the application or data."""
import argparse
import hashlib
import json
import pathlib
import re
import shutil
import subprocess
import zipfile


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def build(version, metadata_path):
    root = pathlib.Path(__file__).resolve().parents[2]
    name = f"yanmew-plugin-devkit-{version}-windows-x64"
    output = root / "releases" / "plugin-devkit" / version
    if output.exists():
        raise RuntimeError("工具包版本输出已存在，禁止覆盖；请使用新版本或检查自己的未发布产物")
    cli = root / "src-tauri/target/release/yanmew-plugin.exe"
    if not cli.is_file():
        raise RuntimeError("请先构建 yanmew-plugin.exe")
    stage = output / name
    (stage / "bin").mkdir(parents=True)
    shutil.copy2(cli, stage / "bin/yanmew-plugin.exe")
    for directory in ("sdk", "template"):
        shutil.copytree(root / "plugins" / directory, stage / directory)
    shutil.copy2(root / "LICENSE", stage / "LICENSE")
    shutil.copy2(root / "LICENSE", stage / "template/LICENSE.txt")
    shutil.copy2(root / "plugins/build/devkit-README.md", stage / "README.md")
    readme = stage / "README.md"
    readme.write_text(re.sub(r"devkit-v\d+\.\d+\.\d+", f"devkit-v{version}", readme.read_text(encoding="utf-8")), encoding="utf-8", newline="\n")
    sdk = stage / "sdk/README.md"
    text = sdk.read_text(encoding="utf-8")
    text = re.sub(r"devkit-v\d+\.\d+\.\d+", f"devkit-v{version}", text)
    text = re.sub(r"devkit-\d+\.\d+\.\d+-windows-x64", f"devkit-{version}-windows-x64", text)
    text = text.replace("../build/README.md", "../README.md#打包与校验")
    text = text.replace("../first-party/", "https://github.com/xianyvqiu/yanmew/blob/develop/plugins/first-party/")
    text = text.replace('"./plugins/sdk/mock-host.js"', '"./sdk/mock-host.js"')
    text += "\n主仓库第一方示例为可选参考；主仓库未公开时需要访问权限。本教程的完整猜数字示例、模板和全部接口可独立使用。\n"
    sdk.write_text(text, encoding="utf-8", newline="\n")
    metadata = json.loads(pathlib.Path(metadata_path).read_text(encoding="utf-8-sig"))
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    included, queue = set(), [metadata["resolve"]["root"]]
    while queue:
        identifier = queue.pop()
        if identifier in included:
            continue
        included.add(identifier)
        queue.extend(dep["pkg"] for dep in nodes[identifier]["deps"]
                     if any(kind["kind"] is None for kind in dep["dep_kinds"]))
    notices = ["# 第三方组件声明", "", "CLI 静态链接 Rust 依赖，使用 Windows 系统 DLL；未捆绑系统 DLL、模型或 WebView2。",
               "下列清单保守包含 Windows 目标运行依赖，链接器可能移除其中未使用代码。各组件保留原许可。",
               "许可证文本位于 licenses/；完整、未经修改的对应版本源码可通过每项 crates.io 链接获取。", ""]
    for package in sorted(metadata["packages"], key=lambda p: (p["name"], p["version"])):
        if package["id"] not in included or package["id"] == metadata["resolve"]["root"]:
            continue
        package_root = pathlib.Path(package["manifest_path"]).parent
        files = {p for p in package_root.iterdir() if p.is_file() and re.match(r"^(LICENSE|LICENCE|COPYING|NOTICE|COPYRIGHT)([._-]|$)", p.name, re.I)}
        if package.get("license_file"):
            files.add(package_root / package["license_file"])
        target = stage / "licenses" / f'{package["name"]}-{package["version"]}'
        for file in sorted(files):
            if file.is_file():
                target.mkdir(parents=True, exist_ok=True)
                shutil.copy2(file, target / file.name)
        notices.append(f'- {package["name"]} {package["version"]}：{package.get("license") or "见组件源码"}；[对应源码](https://crates.io/api/v1/crates/{package["name"]}/{package["version"]}/download)')
    sysroot = pathlib.Path(subprocess.check_output(["rustc", "--print", "sysroot"], text=True).strip())
    rust_license_root = sysroot / "share/doc/rust"
    rust_target = stage / "licenses/rust-standard-library"
    rust_target.mkdir(parents=True)
    for file in rust_license_root.glob("COPYRIGHT-library.html"):
        if file.is_file():
            shutil.copy2(file, rust_target / file.name)
    if not list(rust_target.iterdir()):
        raise RuntimeError("缺少 Rust 标准库许可文本")
    notices += ["", "Rust 标准库按 MIT/Apache-2.0 及其随附第三方许可分发；见 licenses/rust-standard-library/。"]
    (stage / "THIRD_PARTY_NOTICES.md").write_text("\n".join(notices) + "\n", encoding="utf-8")
    source_paths = ["src-tauri/src/bin/yanmew-plugin.rs", "src-tauri/src/game_plugins/manifest.rs", "src-tauri/src/game_plugins/package.rs", "src-tauri/Cargo.lock"]
    info = {"version": version, "apiVersion": "1.0.0", "platform": "windows-x64", "minimumWindows": "10",
            "cli": "bin/yanmew-plugin.exe", "cliSha256": sha(cli),
            "sourceCommit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip(),
            "sourceFiles": {p: sha(root / p) for p in source_paths},
            "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip()}
    (stage / "DEVKIT.json").write_text(json.dumps(info, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    checksums = [f"{sha(p)}  {p.relative_to(stage).as_posix()}" for p in sorted(stage.rglob("*")) if p.is_file()]
    (stage / "SHA256SUMS").write_text("\n".join(checksums) + "\n", encoding="utf-8")
    archive = output / f"{name}.zip"
    with zipfile.ZipFile(archive, "x", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as z:
        for file in sorted(stage.rglob("*")):
            if file.is_file():
                entry = zipfile.ZipInfo(f"{name}/{file.relative_to(stage).as_posix()}")
                entry.compress_type = zipfile.ZIP_DEFLATED
                entry.external_attr = 0o100644 << 16
                z.writestr(entry, file.read_bytes())
    (output / f"{name}.zip.sha256").write_text(f"{sha(archive)}  {archive.name}\n", encoding="ascii")
    print(json.dumps({"archive": str(archive), "sha256": sha(archive), "sizeBytes": archive.stat().st_size}))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--version", required=True)
    parser.add_argument("--metadata", required=True)
    args = parser.parse_args()
    if not re.fullmatch(r"\d+\.\d+\.\d+", args.version):
        parser.error("版本须为正式 SemVer")
    build(args.version, args.metadata)
