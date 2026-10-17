// frame <in.png> <out.png> <corner-radius-px> — rounds a capture's corners and
// sets it on a transparent canvas with a hairline edge and a soft drop shadow.
import AppKit

let args = CommandLine.arguments
guard args.count == 4, let image = NSImage(contentsOfFile: args[1]),
      let source = image.cgImage(forProposedRect: nil, context: nil, hints: nil),
      let radius = Double(args[3]) else {
    FileHandle.standardError.write("usage: frame <in.png> <out.png> <radius>\n".data(using: .utf8)!)
    exit(2)
}
let w = Double(source.width), h = Double(source.height)
let pad = 80.0
let canvas = CGRect(x: 0, y: 0, width: w + pad * 2, height: h + pad * 2)
let card = CGRect(x: pad, y: pad + 16, width: w, height: h)
guard let ctx = CGContext(
    data: nil, width: Int(canvas.width), height: Int(canvas.height), bitsPerComponent: 8,
    bytesPerRow: 0, space: CGColorSpace(name: CGColorSpace.sRGB)!,
    bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
) else { exit(1) }
let shape = CGPath(roundedRect: card, cornerWidth: radius, cornerHeight: radius, transform: nil)

ctx.saveGState()
ctx.setShadow(offset: CGSize(width: 0, height: -24), blur: 64, color: CGColor(gray: 0, alpha: 0.45))
ctx.addPath(shape)
ctx.setFillColor(CGColor(gray: 0, alpha: 1))
ctx.fillPath()
ctx.restoreGState()

ctx.saveGState()
ctx.addPath(shape)
ctx.clip()
ctx.draw(source, in: card)
ctx.restoreGState()

ctx.addPath(shape)
ctx.setStrokeColor(CGColor(gray: 1, alpha: 0.12))
ctx.setLineWidth(2)
ctx.strokePath()

guard let out = ctx.makeImage(),
      let png = NSBitmapImageRep(cgImage: out).representation(using: .png, properties: [:]) else { exit(1) }
try png.write(to: URL(fileURLWithPath: args[2]))
