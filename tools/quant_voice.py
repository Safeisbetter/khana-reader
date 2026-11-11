# An 8-bit copy of an ONNX model's weight matrices (dynamic quantization of MatMul and Gemm): quant.py IN.onnx OUT.onnx u8|s8
import sys, os, tempfile, onnx
from onnxruntime.quantization import quantize_dynamic, QuantType
src, dst, wt = sys.argv[1], sys.argv[2], sys.argv[3]
m = onnx.load(src)                     # (with its external weights)
del m.graph.value_info[:]              # (shape notes inside the file disagree with what is inferred: dropped, they are recomputed)
tmp = tempfile.mkdtemp()
clean = os.path.join(tmp, "clean.onnx")
onnx.save(m, clean, save_as_external_data=True, all_tensors_to_one_file=True, location="clean.data")
quantize_dynamic(clean, dst, weight_type=QuantType.QInt8 if wt == "s8" else QuantType.QUInt8, op_types_to_quantize=["MatMul", "Gemm"], per_channel=False)
print("made", dst, os.path.getsize(dst))
