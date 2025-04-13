import Foundation
import AppKit
import UniformTypeIdentifiers


struct AppMetadata: Codable, Hashable {
    var name: String
    var path: String
    var pid: Int32?
    var icon: String?
}


class AppHandler {
  static func getInstalledApps() -> [AppMetadata] {
        let applicationDirectories = [
            "/Applications",
            "/System/Applications",
            "/System/Library/CoreServices",
            "/Library/Applications",
            FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Applications").path
        ]
        
        var installedApps: [AppMetadata] = []
        
        for directory in applicationDirectories {
            guard let enumerator = FileManager.default.enumerator(
                at: URL(fileURLWithPath: directory),
                includingPropertiesForKeys: [.isDirectoryKey, .nameKey],
                options: [.skipsHiddenFiles]
            ) else {
                continue
            }
            
            while let fileURL = enumerator.nextObject() as? URL {
                guard let isDirectory = try? fileURL.resourceValues(forKeys: [.isDirectoryKey]).isDirectory,
                      isDirectory == true,
                      fileURL.pathExtension == "app" else {
                    continue
                }
                
                let appName = fileURL.deletingPathExtension().lastPathComponent
                
                // Skip helper apps and system apps
                guard !appName.contains("Helper"),
                      !appName.contains("Agent"),
                      !appName.hasSuffix("Assistant"),
                      !appName.hasPrefix("com."),
                      !appName.hasPrefix("plugin_"),
                      !appName.hasPrefix(".") else {
                    continue
                }
                
                let app = AppMetadata(
                    name: appName,
                    path: fileURL.path,
                    pid: nil,
                    icon: nil
                )
                
                installedApps.append(app)
            }
        }
        
        // Remove duplicates and sort
        return Array(Set(installedApps)).sorted { $0.name < $1.name }
    }
}