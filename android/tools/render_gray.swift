// Test helper (Mac only): a PDF page as a grey picture at 250 dpi, as the reader page sends it for
// OCR (u32 width, u32 height little endian, then the pixels), or with "scan" the pages without text.
//   swift render_gray.swift file.pdf scan          -> page numbers with no text
//   swift render_gray.swift file.pdf N out.gray    -> page N's picture
import Foundation
import PDFKit
let a = CommandLine.arguments
guard let doc = PDFDocument(url: URL(fileURLWithPath: a[1])) else { print("cannot open"); exit(1) }
if a[2] == "scan" {
  var s: [String] = []
  for i in 0..<min(doc.pageCount, 60) { if (doc.page(at: i)?.string ?? "").trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { s.append(String(i + 1)) } }
  print("pages \(doc.pageCount); without text: \(s.joined(separator: " "))"); exit(0)
}
let page = doc.page(at: Int(a[2])! - 1)!
let box = page.bounds(for: .mediaBox)
let scale = 250.0 / 72.0
let w = Int(box.width * scale), h = Int(box.height * scale)
var px = [UInt8](repeating: 255, count: w * h)
let cs = CGColorSpaceCreateDeviceGray()
px.withUnsafeMutableBytes { buf in
  let ctx = CGContext(data: buf.baseAddress, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w, space: cs, bitmapInfo: 0)!
  ctx.setFillColor(gray: 1, alpha: 1); ctx.fill(CGRect(x: 0, y: 0, width: w, height: h))
  ctx.scaleBy(x: scale, y: scale)
  page.draw(with: .mediaBox, to: ctx)
}
var out = Data()
withUnsafeBytes(of: UInt32(w).littleEndian) { out.append(contentsOf: $0) }
withUnsafeBytes(of: UInt32(h).littleEndian) { out.append(contentsOf: $0) }
out.append(contentsOf: px)
try! out.write(to: URL(fileURLWithPath: a[3]))
print("\(w)x\(h)")
