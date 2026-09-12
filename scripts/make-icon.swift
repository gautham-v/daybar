// The app icon's 1024pt master, drawn with AppKit rather than shipped as a
// vector asset: the glyph is the same rounded-stroke figure the menu bar draws,
// so keeping it in code keeps the two from drifting. Run via scripts/make-icon.sh,
// which renders this and folds the sizes into assets/AppIcon.icns.
//
//   swift scripts/make-icon.swift out.png

import AppKit

let S: CGFloat = 1024
let inset: CGFloat = 100
let bodyR: CGFloat = 185

let ink = NSColor(srgbRed: 0.12, green: 0.12, blue: 0.11, alpha: 1)
let accent = NSColor(srgbRed: 0.77, green: 0.33, blue: 0.23, alpha: 1)

func rounded(_ r: NSRect, _ rad: CGFloat) -> NSBezierPath {
    NSBezierPath(roundedRect: r, xRadius: rad, yRadius: rad)
}

func render(_ draw: (NSRect) -> Void) -> NSBitmapImageRep {
    let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: Int(S), pixelsHigh: Int(S),
        bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
        colorSpaceName: .calibratedRGB, bytesPerRow: 0, bitsPerPixel: 0)!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    let body = NSRect(x: inset, y: inset, width: S - 2*inset, height: S - 2*inset)
    let shape = rounded(body, bodyR)
    NSGraphicsContext.current!.cgContext.saveGState()
    shape.addClip()
    NSGradient(colors: [NSColor(srgbRed: 0.976, green: 0.972, blue: 0.960, alpha: 1),
                        NSColor(srgbRed: 0.898, green: 0.890, blue: 0.870, alpha: 1)])!
        .draw(in: body, angle: -90)
    NSGraphicsContext.current!.cgContext.restoreGState()
    // hairline edge so the icon keeps an edge on a white wallpaper
    NSColor(srgbRed: 0, green: 0, blue: 0, alpha: 0.10).setStroke()
    let edge = rounded(body.insetBy(dx: 1.5, dy: 1.5), bodyR - 1.5); edge.lineWidth = 3; edge.stroke()
    draw(body)
    NSGraphicsContext.restoreGraphicsState()
    return rep
}

func daybar(_ body: NSRect) {
    let w: CGFloat = 510, h: CGFloat = 470, stroke: CGFloat = 34
    let f = NSRect(x: body.midX - w/2, y: body.midY - h/2 - 14, width: w, height: h)
    let e = f.insetBy(dx: stroke/2, dy: stroke/2)
    ink.setStroke()
    // the two hangers, above the frame
    for x in [e.minX + e.width*0.28, e.maxX - e.width*0.28] {
        let hp = NSBezierPath()
        hp.move(to: NSPoint(x: x, y: e.maxY + 4))
        hp.line(to: NSPoint(x: x, y: e.maxY + 56))
        hp.lineWidth = stroke; hp.lineCapStyle = .round; hp.stroke()
    }
    let p = rounded(e, 58)
    p.lineWidth = stroke; p.lineJoinStyle = .round; p.stroke()
    // the header rule
    let ruleY = e.maxY - 96
    let r0 = NSBezierPath()
    r0.move(to: NSPoint(x: e.minX, y: ruleY)); r0.line(to: NSPoint(x: e.maxX, y: ruleY))
    r0.lineWidth = stroke; r0.stroke()
    // the month grid below it: 7 columns x 3 rows, one day marked
    let cols = 7, rows = 3
    let padX: CGFloat = 60, padTop: CGFloat = 62, padBottom: CGFloat = 58
    let gw = e.width - 2*padX
    let dx = gw / CGFloat(cols - 1)
    let gTop = ruleY - padTop, gBottom = e.minY + padBottom
    let dy = (gTop - gBottom) / CGFloat(rows - 1)
    let r: CGFloat = 13
    for row in 0..<rows {
        for col in 0..<cols {
            let x = e.minX + padX + CGFloat(col)*dx
            let y = gTop - CGFloat(row)*dy
            let marked = (row == 1 && col == 3)
            (marked ? accent : ink.withAlphaComponent(0.42)).setFill()
            let rr = marked ? r * 2.1 : r
            NSBezierPath(ovalIn: NSRect(x: x-rr, y: y-rr, width: 2*rr, height: 2*rr)).fill()
        }
    }
}

let out = CommandLine.arguments[1]
let rep = render(daybar)
try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: out))
