// Build helper (Mac): the app's font, web/Vazirmatn.woff2, as a TrueType file for Android's PDF
// layout (Android reads TTF/OTF). CoreText opens the WOFF2 and hands back each table as decoded;
// the tables are written into a plain sfnt with a fresh table directory and checksums.
//   swift woff2_to_ttf.swift in.woff2 out.ttf
import CoreText
import Foundation
let a = CommandLine.arguments
let data = try! Data(contentsOf: URL(fileURLWithPath: a[1])) as CFData
guard let desc = CTFontManagerCreateFontDescriptorFromData(data) else { print("not a font"); exit(1) }
let font = CTFontCreateWithFontDescriptor(desc, 12, nil)
// (the array holds the tags themselves as its values, not objects)
let arr = CTFontCopyAvailableTables(font, [])!
let tags = (0..<CFArrayGetCount(arr)).map { UInt32(UInt(bitPattern: CFArrayGetValueAtIndex(arr, $0))) }.sorted()
var tables: [(UInt32, Data)] = []
for t in tags { if let d = CTFontCopyTable(font, t, []) { tables.append((t, d as Data)) } }
func be32(_ v: UInt32) -> Data { withUnsafeBytes(of: v.bigEndian) { Data($0) } }
func be16(_ v: UInt16) -> Data { withUnsafeBytes(of: v.bigEndian) { Data($0) } }
func checksum(_ d: Data) -> UInt32 {
  var p = d; while p.count % 4 != 0 { p.append(0) }
  var s: UInt32 = 0
  p.withUnsafeBytes { b in for i in stride(from: 0, to: b.count, by: 4) { s = s &+ (UInt32(b[i]) << 24 | UInt32(b[i+1]) << 16 | UInt32(b[i+2]) << 8 | UInt32(b[i+3])) } }
  return s
}
let n = UInt16(tables.count)
var pow2: UInt16 = 1, log2: UInt16 = 0
while pow2 * 2 <= n { pow2 *= 2; log2 += 1 }
let hasCFF = tables.contains { $0.0 == 0x43464620 }
var head = be32(hasCFF ? 0x4F54544F : 0x00010000) + be16(n) + be16(pow2 * 16) + be16(log2) + be16(n * 16 - pow2 * 16)
var body = Data()
var offset = UInt32(12 + 16 * Int(n))
var headAt = -1
for (t, d0) in tables {
  var d = d0
  if t == 0x68656164 { headAt = Int(offset); d.replaceSubrange(8..<12, with: be32(0)) } // head: checksum adjustment 0 for now
  head += be32(t) + be32(checksum(d)) + be32(offset) + be32(UInt32(d.count))
  body += d
  while body.count % 4 != 0 { body.append(0) }
  offset = UInt32(12 + 16 * Int(n) + body.count)
}
var out = head + body
if headAt >= 0 { out.replaceSubrange(headAt + 8..<headAt + 12, with: be32(0xB1B0AFBA &- checksum(out))) }
try! out.write(to: URL(fileURLWithPath: a[2]))
print("\(tables.count) tables, \(out.count) bytes")
