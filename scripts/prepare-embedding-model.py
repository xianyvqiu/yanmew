"""开发期导出工具；最终应用不依赖 Python。固定官方权重，禁止运行期下载。"""
import hashlib
import json
import pathlib
import shutil
import sys
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / ".test-temp/model-build"))
import numpy as np
import onnxruntime as ort
import torch
from transformers import AutoModel, AutoTokenizer

REVISION = "7999e1d3359715c523056ef9478215996d62a620"
SOURCE = ROOT / ".test-temp/model-source"
OUTPUT = ROOT / "resources/models/bge-small-zh-v1.5"
SOURCE.mkdir(parents=True, exist_ok=True)
OUTPUT.mkdir(parents=True, exist_ok=True)

for name in ["config.json", "model.safetensors", "tokenizer.json", "tokenizer_config.json", "special_tokens_map.json", "vocab.txt", "README.md"]:
    path = SOURCE / name
    if not path.exists():
        print("下载", name, flush=True)
        urllib.request.urlretrieve(f"https://huggingface.co/BAAI/bge-small-zh-v1.5/resolve/{REVISION}/{name}", path)
for name in ["config.json", "tokenizer.json", "tokenizer_config.json", "special_tokens_map.json", "vocab.txt"]:
    shutil.copy2(SOURCE / name, OUTPUT / name)
shutil.copy2(SOURCE / "README.md", OUTPUT / "MODEL_CARD.md")
LICENSE_SOURCE = "https://raw.githubusercontent.com/FlagOpen/FlagEmbedding/fd1a2bdf69488ffebe0327999d4400d8c8058a0b/LICENSE"
if not (OUTPUT / "MODEL_LICENSE").exists():
    urllib.request.urlretrieve(LICENSE_SOURCE, OUTPUT / "MODEL_LICENSE")
LICENSE_SOURCE = "https://raw.githubusercontent.com/FlagOpen/FlagEmbedding/fd1a2bdf69488ffebe0327999d4400d8c8058a0b/LICENSE"
if not (OUTPUT / "MODEL_LICENSE").exists():
    urllib.request.urlretrieve(LICENSE_SOURCE, OUTPUT / "MODEL_LICENSE")

model = AutoModel.from_pretrained(SOURCE, local_files_only=True, attn_implementation="eager").eval()
tokenizer = AutoTokenizer.from_pretrained(SOURCE, local_files_only=True)

class Encoder(torch.nn.Module):
    def forward(self, input_ids, attention_mask, token_type_ids):
        hidden = model(input_ids=input_ids, attention_mask=attention_mask, token_type_ids=token_type_ids).last_hidden_state[:, 0]
        return torch.nn.functional.normalize(hidden, p=2, dim=1)

encoder = Encoder()
# 注册权重为子模块，导出为常量参数而不是捕获外部梯度张量。
encoder.add_module("model", model)
encoder.eval()
sample = tokenizer("为这个句子生成表示以用于检索相关文章：之前去哪里旅行", return_tensors="pt")
with torch.no_grad():
    torch.onnx.export(encoder, tuple(sample[k] for k in ["input_ids", "attention_mask", "token_type_ids"]), str(OUTPUT / "model.onnx"),
        input_names=["input_ids", "attention_mask", "token_type_ids"], output_names=["embedding"],
        dynamic_axes={k: {0: "batch", 1: "sequence"} for k in ["input_ids", "attention_mask", "token_type_ids"]},
        opset_version=17, do_constant_folding=True, dynamo=False)
session = ort.InferenceSession(str(OUTPUT / "model.onnx"), providers=["CPUExecutionProvider"])
texts = ["用户读完三体，最喜欢史强。", "为这个句子生成表示以用于检索相关文章：大史是谁", "今天一起讨论了杭州旅行和桂花糕伴手礼。"]
errors = []
for text in texts:
    inputs = tokenizer(text, return_tensors="pt")
    with torch.no_grad():
        reference = encoder(**inputs).numpy()
    actual = session.run(None, {k: v.numpy() for k, v in inputs.items()})[0]
    error = float(np.max(np.abs(actual - reference)))
    assert actual.shape == (1, 512) and error < 1e-5, (actual.shape, error)
    errors.append(error)
print("ONNX 与官方 FP32 输出最大差异", max(errors), flush=True)

runtime = pathlib.Path(ort.__file__).parent
shutil.copy2(runtime / "capi/onnxruntime.dll", OUTPUT / "onnxruntime.dll")
for name in ["LICENSE", "ThirdPartyNotices.txt"]:
    shutil.copy2(runtime / name, OUTPUT / ("ONNXRUNTIME_" + name))
manifest = {
    "model": "BAAI/bge-small-zh-v1.5", "revision": REVISION, "dimensions": 512, "maxSequence": 512,
    "pooling": "CLS", "normalize": "L2", "queryPrefix": "为这个句子生成表示以用于检索相关文章：",
    "runtime": "1.22.0", "ort": "2.0.0-rc.10", "tokenizers": "0.21.4",
    "export": "official safetensors; torch 2.6.0 CPU / transformers 4.49.0 / ONNX opset 17; FP32 CLS + L2",
    "referenceMaxAbsError": max(errors), "files": {},
    "modelLicenseSource": LICENSE_SOURCE,
    "modelLicenseSource": LICENSE_SOURCE,
}
for path in sorted(OUTPUT.iterdir()):
    if path.name != "manifest.json":
        manifest["files"][path.name] = hashlib.sha256(path.read_bytes()).hexdigest()
(OUTPUT / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print("资源已导出", OUTPUT, flush=True)
