# Credits and licences

Khana Reader stands on the work of many people. Thank you to all of them.

## Special thanks: GrapheneOS

The way this app protects its users follows the [GrapheneOS PDF Viewer](https://github.com/GrapheneOS/PdfViewer). Its source
was read closely and its design followed: pdf.js in a locked-down, sandboxed frame, a strict content security policy, a
WebView with network and file access turned off, no internet permission on Android, and reproducible Android builds.
Specifically, taken from its source (release 33): `targetSandboxVersion` 2 and memory tagging in the Android manifest; a web view with
network loads, file access and content access off, no cache and no cookies; every request answered inside the app; a strict
Content-Security-Policy on the page and one on the script that pdf.js runs in the background; the same list of 29 browser powers,
all switched off; no JavaScript evaluation; no system fonts; a limit of 8,388,608 pixels on a drawn page; a minimum web view
version of 133; no internet and no storage permission; and reproducible Android builds. Where this app differs, the README says so.
Any mistakes in this app are our own, and GrapheneOS does not endorse or support it.

The GrapheneOS PDF Viewer is under the MIT licence:

> Copyright © 2017-2025 GrapheneOS
>
> Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation
> files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy,
> modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the
> Software is furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES
> OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
> LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
> CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

## Models carried by the apps

| Component | Used for | By | Licence |
|---|---|---|---|
| [NLLB-200](https://huggingface.co/facebook/nllb-200-3.3B) 3.3B (Mac) and distilled 600M (Android) | Translation | Meta AI | CC BY-NC 4.0, **non-commercial** |
| [CTranslate2 conversion of NLLB 3.3B](https://huggingface.co/OpenNMT/nllb-200-3.3B-ct2-int8) | Translation model in the format the app reads | OpenNMT | CC BY-NC 4.0, **non-commercial** |
| [pocket-tts-farsi-v2](https://huggingface.co/mehdi-hf/pocket-tts-farsi-v2), with its news voice sample | Farsi speech | mehdi-hf | CC BY-NC 4.0, **non-commercial** |
| Five voice samples, `Andrea`, `Aryobe`, `Imran_from_I_India`, `taiyo` and `Roscoe`, from [kyutai/tts-voices](https://huggingface.co/kyutai/tts-voices) (`voice-donations/`); the last three prepared by the app as it prepares a recorded voice (first 5 s of speech, loudness evened) from the originals with SHA-256 b4a18e3f…, d32288c1…, 1a717e3a… | The voices «مدل صدا ۲» to «مدل صدا ۶» | Volunteers of the Unmute Voice Donation Project, collected by Kyutai | CC0 (public domain); the files equal the SHA-256 Hugging Face lists |
| [Pocket TTS](https://huggingface.co/kyutai/pocket-tts) | The speech model's design and its Mimi codec | Kyutai | CC BY 4.0 |
| [Vosk small Farsi model](https://alphacephei.com/vosk/models) | Language data and listening checks | Alpha Cephei | Apache 2.0 |
| [Whisper large-v3-turbo](https://huggingface.co/openai/whisper-large-v3-turbo), CTranslate2 format by [deepdml](https://huggingface.co/deepdml/faster-whisper-large-v3-turbo-ct2) | Speech to text (Mac) | OpenAI; conversion by deepdml | MIT |
| [PP-OCRv5 Arabic-script recogniser](https://github.com/PaddlePaddle/PaddleOCR) | Reading scanned pages | PaddlePaddle | Apache 2.0 |
| [CMUdict](http://www.speech.cs.cmu.edu/cgi-bin/cmudict) | English pronunciation | Carnegie Mellon University | BSD-style, Copyright (C) 1993-2015 Carnegie Mellon University |

These models were changed only by converting them to run on a device. Their authors do not endorse this app.

### How the model files were checked

Each file was compared with its publisher's own copy (November 2026):

- **Translation (NLLB 600M, 1.3B, 3.3B) and dictation (Whisper):** the same SHA-256 as Hugging Face lists; the 600M file was also
  converted again from Meta's original and came out byte for byte the same.
- **Listening model (Vosk):** every file identical to the publisher's zip.
- **OCR model:** the same SHA-256 as PaddlePaddle's file on Hugging Face.
- **Voice and pronunciation models:** these are our own conversions to ONNX, so no published checksum exists for them. Their
  weights were compared number by number with the published originals (`mehdi-hf/pocket-tts-farsi-v2`,
  `mehdi-hf/Homo-GE2PE-Persian-HF`) and are equal; the files contain only standard ONNX operations, no custom code.
- **The voice model on Android and Windows** is an 8-bit copy of the same file (its weight matrices stored as 8-bit numbers,
  made with ONNX Runtime's own quantization tool): a quarter of the size and faster on a phone. The Mac keeps the 32-bit file.
- **ONNX Runtime and Vosk libraries (Mac):** the program body is identical to the publisher's download (only the code signature differs).

[`MODELS.sha256`](MODELS.sha256) lists the SHA-256 of every model file and ready-made library in the apps, so anyone can check that a
download carries exactly these files. Model files are data (numbers and tables); the app does not run code from them.

## Software

| Component | Used for | By | Licence |
|---|---|---|---|
| [pdf.js](https://github.com/mozilla/pdf.js) 6.5.438 | Opening PDFs. One function in `pdf.worker.mjs` (`runBidiTransform`) was changed to keep right-to-left letters in order | Mozilla | Apache 2.0, see `web/pdfjs/PDFJS_LICENSE` |
| [CTranslate2](https://github.com/OpenNMT/CTranslate2) | Runs the translation model | OpenNMT | MIT, see `vendor/ct2include/LICENSE.ctranslate2` |
| [ONNX Runtime](https://github.com/microsoft/onnxruntime) | Runs the speech and OCR models | Microsoft | MIT |
| [Vosk](https://github.com/alphacep/vosk-api) | Language data and listening checks | Alpha Cephei | Apache 2.0 |
| [LAME](https://lame.sourceforge.io/) | Saving MP3s | The LAME project | LGPL, source and licence in `vendor/lame` |
| [Vazirmatn](https://github.com/rastikerdar/vazirmatn) | The Farsi font | Saber Rastikerdar | SIL Open Font License 1.1 |
| Rust libraries | See `Cargo.lock` for the full list and versions | Their authors | Mainly MIT and Apache 2.0, some Zlib, Unicode-3.0 and MPL-2.0 |

## Using a voice

The speech model's makers forbid copying anyone's voice without their explicit, lawful consent, and any use meant to deceive.
«صدای من» is for your own voice only.
