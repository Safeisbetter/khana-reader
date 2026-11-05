// Minimal stand-in for half_float::half (the 16-bit float type CTranslate2's headers name). Khana
// translates with int8 on the CPU and never touches 16-bit floats; only the type's size and layout
// (one uint16_t) must match the library's.
#pragma once
#include <cstdint>
namespace half_float {
struct half {
  std::uint16_t data_ = 0;
  half() = default;
  explicit half(float) {}
  operator float() const { return 0.0f; }
};
}
