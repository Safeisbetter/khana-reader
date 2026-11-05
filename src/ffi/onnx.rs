//! The one ONNX Runtime call the `ort` crate has no safe form for.

use std::ptr::NonNull;

/// An empty float tensor (one dimension is 0: no voice frames yet, no text). ONNX Runtime allocates
/// it itself, because ort's safe `from_array` refuses zero-length dimensions.
pub fn empty_f32(shape: &[usize]) -> ort::DynValue {
    let dims: Vec<i64> = shape.iter().map(|x| *x as i64).collect();
    let alloc = ort::Allocator::default();
    let mut ptr: *mut ort_sys::OrtValue = std::ptr::null_mut();
    // SAFETY: the API table belongs to the loaded runtime; `dims` lives through the call and its
    // length is passed; `ptr` is valid for writing. A null status means success, and the new value
    // is handed to ort's DynValue, which owns and releases it.
    unsafe {
        let api = ort::api().as_ref();
        let st = (api.CreateTensorAsOrtValue.expect("ONNX Runtime API"))(alloc.ptr(), dims.as_ptr(), dims.len() as _,
            ort_sys::ONNXTensorElementDataType::ONNX_TENSOR_ELEMENT_DATA_TYPE_FLOAT, &mut ptr);
        assert!(st.is_null(), "cannot make an empty tensor");
        ort::DynValue::from_ptr(NonNull::new(ptr).expect("empty tensor"), None)
    }
}
