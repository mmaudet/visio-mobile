import SwiftUI

struct VisioLogo: View {
    var size: CGFloat = 64

    // French flag colors
    private let blueFR = Color(red: 0, green: 0x23 / 255.0, blue: 0x95 / 255.0)
    private let whiteFR = Color.white
    private let redFR = Color(red: 0xED / 255.0, green: 0x29 / 255.0, blue: 0x39 / 255.0)

    var body: some View {
        Canvas { context, canvasSize in
            let w = canvasSize.width
            let h = canvasSize.height

            drawTricoloreBackground(context: context, w: w, h: h)
            drawCameraBody(context: context, w: w, h: h)
            drawWifiArcs(context: context, w: w, h: h)
        }
        .frame(width: size, height: size)
    }

    private func drawTricoloreBackground(context: GraphicsContext, w: CGFloat, h: CGFloat) {
        let third = w / 3.0
        // Blue stripe
        context.fill(Path(CGRect(x: 0, y: 0, width: third, height: h)), with: .color(blueFR))
        // White stripe
        context.fill(Path(CGRect(x: third, y: 0, width: third, height: h)), with: .color(whiteFR))
        // Red stripe
        context.fill(Path(CGRect(x: third * 2, y: 0, width: third, height: h)), with: .color(redFR))
    }

    private func drawCameraBody(context: GraphicsContext, w: CGFloat, h: CGFloat) {
        let bodyColor = Color(red: 0x1D / 255.0, green: 0x1D / 255.0, blue: 0x1F / 255.0)
        // Camera body rectangle
        let bodyLeft = w * 0.10
        let bodyTop = h * 0.30
        let bodyRight = w * 0.62
        let bodyBottom = h * 0.70
        context.fill(
            Path(CGRect(x: bodyLeft, y: bodyTop, width: bodyRight - bodyLeft, height: bodyBottom - bodyTop)),
            with: .color(bodyColor)
        )
        // Camera lens (trapezoid pointing right)
        var lensPath = Path()
        lensPath.move(to: CGPoint(x: bodyRight, y: bodyTop + (bodyBottom - bodyTop) * 0.20))
        lensPath.addLine(to: CGPoint(x: w * 0.78, y: bodyTop))
        lensPath.addLine(to: CGPoint(x: w * 0.78, y: bodyBottom))
        lensPath.addLine(to: CGPoint(x: bodyRight, y: bodyBottom - (bodyBottom - bodyTop) * 0.20))
        lensPath.closeSubpath()
        context.fill(lensPath, with: .color(bodyColor))
        // Lens circle (recording indicator)
        let lensRadius = (bodyBottom - bodyTop) * 0.14
        let lensCx = (bodyLeft + bodyRight) / 2.0
        let lensCy = (bodyTop + bodyBottom) / 2.0
        context.fill(
            Path(ellipseIn: CGRect(x: lensCx - lensRadius, y: lensCy - lensRadius, width: lensRadius * 2, height: lensRadius * 2)),
            with: .color(.white)
        )
    }

    private func drawWifiArcs(context: GraphicsContext, w: CGFloat, h: CGFloat) {
        let strokeWidth = w * 0.035
        let centerX = w * 0.82
        let centerY = h * 0.55

        // Three arcs (wifi signal)
        for i in 1...3 {
            let radius = w * 0.06 * CGFloat(i)
            var arcPath = Path()
            arcPath.addArc(
                center: CGPoint(x: centerX, y: centerY),
                radius: radius,
                startAngle: .degrees(-135),
                endAngle: .degrees(-45),
                clockwise: false
            )
            context.stroke(
                arcPath,
                with: .color(.white),
                style: StrokeStyle(lineWidth: strokeWidth, lineCap: .round)
            )
        }

        // Dot at center
        context.fill(
            Path(ellipseIn: CGRect(x: centerX - strokeWidth * 0.8, y: centerY - strokeWidth * 0.8, width: strokeWidth * 1.6, height: strokeWidth * 1.6)),
            with: .color(.white)
        )
    }
}

#Preview {
    VisioLogo(size: 120)
        .padding()
        .background(Color.black)
}
