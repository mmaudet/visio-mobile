import SwiftUI

struct VisioLogo: View {
    var size: CGFloat = 64

    var body: some View {
        if let uiImage = UIImage(named: "AppLogo") {
            Image(uiImage: uiImage)
                .resizable()
                .aspectRatio(contentMode: .fit)
                .frame(width: size, height: size)
                .clipShape(RoundedRectangle(cornerRadius: size * 0.2237, style: .continuous))
        } else {
            Image(systemName: "video.fill")
                .resizable()
                .aspectRatio(contentMode: .fit)
                .frame(width: size, height: size)
                .foregroundColor(.blue)
        }
    }
}

#Preview {
    VisioLogo(size: 120)
        .padding()
        .background(Color.black)
}
