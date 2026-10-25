// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "Lyrical",
    platforms: [.macOS(.v14)],
    targets: [
        .executableTarget(
            name: "Lyrical",
            path: "Sources/Lyrical",
            swiftSettings: [.swiftLanguageMode(.v5)],
            linkerSettings: [
                .linkedFramework("ShazamKit"),
                .linkedFramework("ScreenCaptureKit"),
                .linkedFramework("ServiceManagement"),
            ]
        )
    ]
)
