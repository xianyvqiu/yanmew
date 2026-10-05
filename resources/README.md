# 本地 Embedding 资源

`models/bge-small-zh-v1.5/` 是 Windows x64 便携版所需资源，应用不依赖 Python、CUDA 或独立模型服务。默认 CPU 推理，最多使用 4 个逻辑线程，单实例按需加载，空闲不自旋。

模型为官方 `BAAI/bge-small-zh-v1.5` revision `7999e1d3359715c523056ef9478215996d62a620` 的 FP32 导出，512 维、最大序列 512，CLS 池化与 L2 归一化。查询前缀是“为这个句子生成表示以用于检索相关文章：”，记忆标题/正文不添加查询前缀；超长输入不截断，只保留全文检索。

`manifest.json` 锁定资源 SHA-256、版本、预处理、导出方法和许可证来源。Rust 使用 `ort=2.0.0-rc.10`、`tokenizers=0.21.4`，绝对路径加载随包的 ONNX Runtime `1.22.0` DLL。模型文件 94,847,655 字节，资源目录约 105.5 MiB；发布脚本复制清单内资源及 manifest，用户数据和开发工具不随包复制。

开发模式显式读取项目资源目录。发布模式读取可执行文件旁的 `resources/models/bge-small-zh-v1.5/`。准备便携版暂存目录后，在项目根目录执行：

```powershell
./scripts/copy-embedding-resources.ps1 -Destination <便携版暂存目录>
```

完整便携版仍须由最终发布阶段加入 exe 和其他应用资源；本脚本只负责 Embedding 资源。模型及运行库不进入用户备份，向量缓存可重建。

开发期复现导出：使用 Python 3.12、CPU torch 2.6.0、transformers 4.49.0、onnx 1.17.0、onnxruntime 1.22.0、safetensors 0.5.3，安装到项目临时目录 `my_doc/model-build/`，再运行 `scripts/prepare-embedding-model.py`。脚本下载固定 revision 的官方 safetensors，导出 ONNX opset 17，并比较三个样本的官方 FP32/ONNX 输出，最大绝对误差必须 <1e-5。首次结果为约 3.13e-7。工具目录和原始权重是开发期材料，应用运行不使用它们。

一次 Rust/Windows CPU 验证：Intel Core i7-12700H，20 个逻辑 CPU，推理使用 4 线程，debug 测试进程。加载（含资源校验）1,911 ms，首条短样本 4.72 ms，后续三条平均 3.28 ms；工作集由 6.17 MiB 到加载后 181.95 MiB，测试峰值 184.48 MiB。此数据是本机短样本测量，不是所有机器的速度或完整应用内存承诺。相关查询余弦得分 0.620–0.666，无关查询最高 0.261，首版相关阈值 0.60；检索排名融合不是概率。

缺失、校验失败和推理错误只让 Embedding 局部不可用，不自动上传、不让桌宠休眠；修复后测试当前模型或重启恢复。资源变更时重新验证，常规开发无需反复跑资源测试。

许可证：模型官方标注 MIT，保留 `MODEL_CARD.md` 与 `MODEL_LICENSE`；ONNX Runtime 保留 `ONNXRUNTIME_LICENSE` 与第三方通知。固定来源与每份文件哈希见 manifest。

以上资源验证的 Windows 系统版本为 10.0.26200.0。
