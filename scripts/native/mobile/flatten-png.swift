// Rewrites PNG files as opaque RGB, compositing onto a solid colour.
//
//   swift scripts/native/mobile/flatten-png.swift <#RRGGBB> <file.png>...
//
// App Store Connect rejects an app whose 1024px icon has an alpha channel
// (ITMS-90717), and every icon generator in this repo writes RGBA.
// `vp run native mobile ios icons` runs this over the iOS icon set.
import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data((message + "\n").utf8))
    exit(1)
}

let arguments = CommandLine.arguments.dropFirst()
guard let colour = arguments.first, arguments.count >= 2 else {
    fail("usage: flatten-png.swift <#RRGGBB> <file.png>...")
}
let hex = colour.hasPrefix("#") ? String(colour.dropFirst()) : colour
guard hex.count == 6, let rgb = UInt32(hex, radix: 16) else {
    fail("background must be #RRGGBB, got \(colour)")
}
let background = CGColor(
    srgbRed: CGFloat((rgb >> 16) & 0xFF) / 255,
    green: CGFloat((rgb >> 8) & 0xFF) / 255,
    blue: CGFloat(rgb & 0xFF) / 255,
    alpha: 1
)
guard let space = CGColorSpace(name: CGColorSpace.sRGB) else { fail("sRGB colour space unavailable") }

for path in arguments.dropFirst() {
    let url = URL(fileURLWithPath: path)
    guard let source = CGImageSourceCreateWithURL(url as CFURL, nil),
          let image = CGImageSourceCreateImageAtIndex(source, 0, nil)
    else { fail("cannot read \(path)") }

    // `noneSkipLast` keeps a padding byte in memory but tells ImageIO there is
    // no alpha, so the PNG is written as colour type 2 (RGB).
    guard let context = CGContext(
        data: nil,
        width: image.width,
        height: image.height,
        bitsPerComponent: 8,
        bytesPerRow: 0,
        space: space,
        bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue
    ) else { fail("cannot create a bitmap for \(path)") }
    let bounds = CGRect(x: 0, y: 0, width: image.width, height: image.height)
    context.setFillColor(background)
    context.fill(bounds)
    context.draw(image, in: bounds)

    guard let flattened = context.makeImage(),
          let destination = CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil)
    else { fail("cannot write \(path)") }
    CGImageDestinationAddImage(destination, flattened, nil)
    guard CGImageDestinationFinalize(destination) else { fail("cannot write \(path)") }
    print("flattened \(path)")
}
