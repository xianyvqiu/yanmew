# 本地抠图资源

U²-Net 通用模型，原作者 Xuebin Qin 等；代码及模型采用 Apache-2.0。模型来自 rembg 官方发布版本，MD5 为 60024c5c889badc19c04ad937298a77b，SHA-256 见 manifest.json。

项目：https://github.com/xuebinqin/U-2-Net
模型：https://github.com/danielgatis/rembg/releases/download/v0.0.0/u2net.onnx

预处理及掩膜归一化参考 rembg 的 U2NetSession（MIT，Daniel Gatis）。本应用复用已有 ONNX Runtime 在本机推理，不运行或安装 Python/rembg 服务。许可证原文随资源保留。
