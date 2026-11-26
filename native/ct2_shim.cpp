// A small C layer over CTranslate2 4.5.0 for offline NLLB-200 translation and Whisper STT.
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <limits>
#include <memory>
#include <string>
#include <vector>
#ifdef __ANDROID__
extern "C" void omp_set_num_threads(int);
#endif
#include "ctranslate2/translator.h"
#include "ctranslate2/models/whisper.h"

namespace {
void zero_mem(void* p, size_t n) {
  if (!p || n == 0) return;
  volatile unsigned char* v = static_cast<volatile unsigned char*>(p);
  while (n--) *v++ = 0;
}

void zero_string(std::string& s) {
  if (!s.empty()) zero_mem(&s[0], s.size());
  s.clear();
}

void write_err(char* err, int errlen, const char* msg) {
  if (!err || errlen <= 0) return;
  const char* safe = msg ? msg : "unknown error";
  size_t max_copy = static_cast<size_t>(errlen - 1);
  std::strncpy(err, safe, max_copy);
  err[max_copy] = '\0';
}
}  // namespace

extern "C" {

void* ct2_new(const char* path, int intra_threads, char* err, int errlen) {
  if (!path || path[0] == '\0') {
    write_err(err, errlen, "missing model path");
    return nullptr;
  }
  try {
    int n = intra_threads > 0 ? intra_threads : 1;
#ifdef __ANDROID__
    setenv("KMP_BLOCKTIME", "0", 0);
    setenv("OMP_WAIT_POLICY", "PASSIVE", 0);
    omp_set_num_threads(n);
#endif
    ctranslate2::ReplicaPoolConfig cfg;
    cfg.num_threads_per_replica = n;
    cfg.max_queued_batches = 0;
    auto* t = new ctranslate2::Translator(path, ctranslate2::Device::CPU, ctranslate2::ComputeType::INT8, {0}, false, cfg);
    return t;
  } catch (const std::exception& e) {
    write_err(err, errlen, e.what());
    return nullptr;
  } catch (...) {
    write_err(err, errlen, "translator initialization failed");
    return nullptr;
  }
}

void ct2_free(void* t) { delete static_cast<ctranslate2::Translator*>(t); }

// tokens: sentences separated by '\n', tokens by ' '. prefix: one target-prefix token for all.
// Returns a malloc'd string in the same form (the best hypothesis of each), or NULL.
char* ct2_translate(void* t, const char* tokens, const char* prefix, int beam, int max_batch, int max_len, float rep_pen) {
  if (!t || !tokens || !prefix || beam <= 0 || max_batch < 0 || max_len <= 0) return nullptr;
  std::string s(tokens);
  std::string out;
  try {
    std::vector<std::vector<std::string>> src;
    size_t start = 0;
    while (start <= s.size()) {
      size_t end = s.find('\n', start);
      if (end == std::string::npos) end = s.size();
      std::string line = s.substr(start, end - start);
      std::vector<std::string> toks;
      size_t a = 0;
      while (a < line.size()) {
        size_t b = line.find(' ', a);
        if (b == std::string::npos) b = line.size();
        if (b > a) toks.push_back(line.substr(a, b - a));
        a = b + 1;
      }
      src.push_back(std::move(toks));
      zero_string(line);
      start = end + 1;
      if (end == s.size()) break;
    }
    std::vector<std::vector<std::string>> pre(src.size(), std::vector<std::string>{prefix});
    ctranslate2::TranslationOptions o;
    o.beam_size = static_cast<size_t>(beam);
    o.max_decoding_length = static_cast<size_t>(max_len);
    o.repetition_penalty = rep_pen;
    auto res = static_cast<ctranslate2::Translator*>(t)->translate_batch(src, pre, o, static_cast<size_t>(max_batch));
    for (size_t i = 0; i < res.size(); i++) {
      if (i) out += '\n';
      if (res[i].hypotheses.empty()) continue;
      const auto& h = res[i].hypotheses.at(0);
      for (size_t k = 0; k < h.size(); k++) {
        if (k) out += ' ';
        out += h[k];
      }
    }
    for (auto& row : src) {
      for (auto& tok : row) zero_string(tok);
    }
    zero_string(s);
    char* r = static_cast<char*>(std::malloc(out.size() + 1));
    if (!r) {
      zero_string(out);
      return nullptr;
    }
    std::memcpy(r, out.c_str(), out.size() + 1);
    zero_string(out);
    return r;
  } catch (...) {
    zero_string(s);
    zero_string(out);
    return nullptr;
  }
}

// ---- Whisper (speech to text): features in, the best hypothesis' tokens out
void* wh_new(const char* path, int intra_threads, char* err, int errlen) {
  if (!path || path[0] == '\0') {
    write_err(err, errlen, "missing model path");
    return nullptr;
  }
  try {
    ctranslate2::ReplicaPoolConfig cfg;
    cfg.num_threads_per_replica = intra_threads > 0 ? intra_threads : 1;
    cfg.max_queued_batches = 0;
    return new ctranslate2::models::Whisper(path, ctranslate2::Device::CPU, ctranslate2::ComputeType::INT8, {0}, false, cfg);
  } catch (const std::exception& e) {
    write_err(err, errlen, e.what());
    return nullptr;
  } catch (...) {
    write_err(err, errlen, "whisper initialization failed");
    return nullptr;
  }
}

void wh_free(void* w) { delete static_cast<ctranslate2::models::Whisper*>(w); }

// mel: n_mels x frames floats (row per mel band). prompt: the start tokens, separated by ' '.
// Returns the tokens of the best hypothesis separated by ' ', or NULL.
char* wh_transcribe(void* w, const float* mel, int n_mels, int frames, const char* prompt, int beam, int max_len) {
  if (!w || !mel || !prompt || n_mels <= 0 || frames <= 0 || beam <= 0 || max_len <= 0) return nullptr;
  size_t umels = static_cast<size_t>(n_mels);
  size_t uframes = static_cast<size_t>(frames);
  if (umels > std::numeric_limits<size_t>::max() / uframes) return nullptr;
  std::vector<float> data;
  std::string out;
  try {
    data.assign(mel, mel + umels * uframes);
    ctranslate2::StorageView features(ctranslate2::Shape{1, n_mels, frames}, data.data(), ctranslate2::Device::CPU);
    std::vector<std::string> p;
    std::string s(prompt);
    size_t a = 0;
    while (a < s.size()) {
      size_t b = s.find(' ', a);
      if (b == std::string::npos) b = s.size();
      if (b > a) p.push_back(s.substr(a, b - a));
      a = b + 1;
    }
    ctranslate2::models::WhisperOptions o;
    o.beam_size = static_cast<size_t>(beam);
    o.max_length = static_cast<size_t>(max_len);
    o.suppress_blank = true;
    o.repetition_penalty = 1.0f;
    o.no_repeat_ngram_size = 0;
    auto futs = static_cast<ctranslate2::models::Whisper*>(w)->generate(features, {p}, o);
    if (futs.empty()) {
      zero_mem(data.data(), data.size() * sizeof(float));
      return nullptr;
    }
    auto res = futs.at(0).get();
    if (!res.sequences.empty()) {
      const auto& h = res.sequences.at(0);
      for (size_t k = 0; k < h.size(); k++) {
        if (k) out += ' ';
        out += h[k];
      }
    }
    zero_mem(data.data(), data.size() * sizeof(float));
    char* r = static_cast<char*>(std::malloc(out.size() + 1));
    if (!r) {
      zero_string(out);
      return nullptr;
    }
    std::memcpy(r, out.c_str(), out.size() + 1);
    zero_string(out);
    return r;
  } catch (...) {
    if (!data.empty()) zero_mem(data.data(), data.size() * sizeof(float));
    zero_string(out);
    return nullptr;
  }
}

void ct2_free_str(char* s) {
  if (!s) return;
  zero_mem(s, std::strlen(s));
  std::free(s);
}
}
