import AppKit
import SwiftUI

#if DESIGN_PREVIEW
/// `docs/screenshots.sh` runs the design preview against fixture cards. Once
/// the first snapshot lands, the window drops its title bar and its number
/// goes to INSOMNIA_PREVIEW_READY so the script can capture just the popover.
@MainActor
fileprivate enum PreviewCapture {
    static func ready() {
        guard let path = ProcessInfo.processInfo.environment["INSOMNIA_PREVIEW_READY"],
              let window = NSApp.windows.first(where: \.isVisible) else { return }
        window.styleMask = [.borderless]
        window.hasShadow = false
        DispatchQueue.main.asyncAfter(deadline: .now() + 1) {
            try? String(window.windowNumber).write(toFile: path, atomically: true, encoding: .utf8)
        }
    }
}
#endif

@main
struct InsomniaApp: App {
    @StateObject private var model = DashboardModel()

    init() {
        Typography.registerBundledFonts()
    }

    var body: some Scene {
#if DESIGN_PREVIEW
        WindowGroup("Insomnia Design Preview") {
            DashboardView(model: model)
                .onChange(of: model.refreshedAt) { if $0 != nil { PreviewCapture.ready() } }
        }
        .windowResizability(.contentSize)
#else
        MenuBarExtra {
            DashboardView(model: model)
        } label: {
            if model.isAwake {
                Image(nsImage: StatusIcon.awake)
                    .resizable()
                    .frame(width: 18, height: 18)
                    .accessibilityLabel("Insomnia: display awake")
            } else {
                Image(systemName: "moon.fill")
                    .font(.system(size: 15, weight: .medium))
                    .accessibilityLabel("Insomnia: display sleep normal")
            }
        }
        .menuBarExtraStyle(.window)
#endif
    }
}
