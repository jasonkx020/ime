// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "YcInputServer",
    platforms: [.macOS(.v13)],
    products: [
        .executable(name: "YcInputServer", targets: ["YcInputServer"]),
    ],
    targets: [
        .executableTarget(
            name: "YcInputServer",
            path: "Sources/YcInputServer"
        ),
    ]
)
