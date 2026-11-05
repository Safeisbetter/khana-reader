// A small C layer over CTranslate2 4.5.0 (the library inside the Python app's ctranslate2 wheel),
// for NLLB-200 translation exactly as app.py's translate_sentences runs it.
#include <cstring>
#include <memory>
#include <string>
#include <vector>
#include "ctranslate2/translator.h"

extern "C" {

void* ct2_new(const char* path, int intra_threads, char* err, int errlen) {
  try {
    ctranslate2::ReplicaPoolConfig cfg;
    cfg.num_threads_per_replica = intra_threads;
    cfg.max_queued_batches = 0;
    auto* t = new ctranslate2::Translator(path, ctranslate2::Device::CPU, ctranslate2::ComputeType::INT8, {0}, false, cfg);
    return t;
  } catch (const std::exception& e) {
    std::strncpy(err, e.what(), errlen - 1);
    err[errlen - 1] = 0;
    return nullptr;
  }
}

void ct2_free(void* t) { delete static_cast<ctranslate2::Translator*>(t); }

// tokens: sentences separated by '\n', tokens by ' '. prefix: one target-prefix token for all.
// Returns a malloc'd string in the same form (the best hypothesis of each), or NULL.
char* ct2_translate(void* t, const char* tokens, const char* prefix, int beam, int max_batch, int max_len, float rep_pen) {
  try {
    std::vector<std::vector<std::string>> src;
    std::string s(tokens);
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
      src.push_back(toks);
      start = end + 1;
      if (end == s.size()) break;
    }
    std::vector<std::vector<std::string>> pre(src.size(), std::vector<std::string>{prefix});
    ctranslate2::TranslationOptions o;
    o.beam_size = beam;
    o.max_decoding_length = max_len;
    o.repetition_penalty = rep_pen;
    auto res = static_cast<ctranslate2::Translator*>(t)->translate_batch(src, pre, o, max_batch);
    std::string out;
    for (size_t i = 0; i < res.size(); i++) {
      if (i) out += '\n';
      const auto& h = res[i].hypotheses.at(0);
      for (size_t k = 0; k < h.size(); k++) {
        if (k) out += ' ';
        out += h[k];
      }
    }
    char* r = static_cast<char*>(std::malloc(out.size() + 1));
    std::memcpy(r, out.c_str(), out.size() + 1);
    return r;
  } catch (...) {
    return nullptr;
  }
}

void ct2_free_str(char* s) { std::free(s); }
}
