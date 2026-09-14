// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "DmxKit",
    defaultLocalization: "fr",
    platforms: [.macOS(.v10_15), .iOS(.v17)],
    products: [
        .library(name: "DmxKit", targets: ["DmxKit"]),
    ],
    targets: [
        // Généré par scripts/build-apple-xcframework.sh (noyau Rust dmx-ffi).
        .binaryTarget(name: "DmxCoreFFI", path: "Frameworks/DmxCoreFFI.xcframework"),
        .target(
            name: "DmxCore",
            dependencies: ["DmxCoreFFI"],
            path: "Sources/DmxCore",
            linkerSettings: [
                .linkedFramework("Security"),
                .linkedFramework("SystemConfiguration", .when(platforms: [.macOS])),
                .linkedFramework("CoreFoundation"),
                .linkedLibrary("resolv"),
            ]
        ),
        .target(
            name: "DmxKit",
            dependencies: ["DmxCore"],
            path: "Sources/DmxKit",
            resources: [.process("Resources")]
        ),
        .testTarget(
            name: "DmxKitTests",
            dependencies: ["DmxKit"],
            path: "Tests/DmxKitTests"
        ),
    ]
)
