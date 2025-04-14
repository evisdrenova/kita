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
        FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Applications").path
    ]
    
    for directory in applicationDirectories {
        
        var isDirectory: ObjCBool = false
        let exists = FileManager.default.fileExists(atPath: directory, isDirectory: &isDirectory)

        guard exists && isDirectory.boolValue else {
            continue
        }
    }
    
    var installedApps: [AppMetadata] = []
    
    for directory in applicationDirectories {
        
        guard let enumerator = FileManager.default.enumerator(
            at: URL(fileURLWithPath: directory),
            includingPropertiesForKeys: [.isDirectoryKey, .nameKey, .isHiddenKey],
            options: [.skipsPackageDescendants]
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
            
            let app = AppMetadata(
                name: appName,
                path: fileURL.path,
                pid: nil,
                icon: nil
            )
            
            installedApps.append(app)
        }
        
    }

    let uniqueApps = Array(Set(installedApps)).sorted { $0.name < $1.name }
    
    return uniqueApps
}

    // Get running applications
static func getRunningApps() -> [AppMetadata] {
    let runningApps = NSWorkspace.shared.runningApplications
    
    return runningApps
        .compactMap { app -> AppMetadata? in
            guard let bundlePath = app.bundleURL?.path,
                  let appName = app.localizedName else {
                return nil
            }
            
            return AppMetadata(
                name: appName,
                path: bundlePath,
                pid: app.processIdentifier,
                icon: nil
            )
        }
        .sorted { $0.name < $1.name }
}
    
    // Get app icon
    static func getAppIcon(path: String) -> String? {
   let image = NSWorkspace.shared.icon(forFile: path)
        
        // Resize image
        let resizedImage = NSImage(size: NSSize(width: 32, height: 32))
        resizedImage.lockFocus()
        image.draw(in: NSRect(x: 0, y: 0, width: 32, height: 32), 
                   from: NSRect(x: 0, y: 0, width: image.size.width, height: image.size.height), 
                   operation: .sourceOver, 
                   fraction: 1.0)
        resizedImage.unlockFocus()
        
        // Convert to PNG
        guard let tiffRepresentation = resizedImage.tiffRepresentation,
              let bitmapImage = NSBitmapImageRep(data: tiffRepresentation),
              let pngData = bitmapImage.representation(using: .png, properties: [:]) else {
            return generateFallbackIcon(path: path)
        }
        
        // Base64 encode
        let base64String = pngData.base64EncodedString()
        return "data:image/png;base64,\(base64String)"
    }
    
    // Fallback icon generation
    static func generateFallbackIcon(path: String) -> String {
        let name = URL(fileURLWithPath: path).deletingPathExtension().lastPathComponent
        let firstLetter = name.first?.uppercased() ?? "A"
        
        // Generate a pseudo-random color based on the name
        let hash = name.utf8.reduce(0) { $0 + Int($1) }
        let hue = hash % 360
        
        let svg = """
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">
            <rect x="0" y="0" width="64" height="64" rx="12" fill="hsl(\(hue), 70%, 60%)"/>
            <text x="32" y="42" font-family="Arial" font-size="32" font-weight="bold" 
                  text-anchor="middle" fill="white">\(firstLetter)</text>
        </svg>
        """
        
        let base64Svg = svg.data(using: .utf8)?.base64EncodedString() ?? ""
        return "data:image/svg+xml;base64,\(base64Svg)"
    }
    
    // Switch to an app
    static func switchToApp(pid: Int32) -> Bool {
        guard let app = NSRunningApplication(processIdentifier: pid) else {
            return false
        }
        
        return app.activate(options: [.activateAllWindows])
    }
    
    // Force quit an application
    static func forceQuitApplication(pid: Int32) -> Bool {
        guard let app = NSRunningApplication(processIdentifier: pid) else {
            return false
        }
        
        return app.terminate()
    }
    
    // Restart an application
   static func restartApplication(path: String, completion: @escaping (Bool, Error?) -> Void) {
    let url = URL(fileURLWithPath: path)
    
    let configuration = NSWorkspace.OpenConfiguration()
    
    NSWorkspace.shared.openApplication(at: url, configuration: configuration) { app, error in
        if let error = error {
            print("Failed to restart application: \(error)")
            completion(false, error)
            return
        }
        
        guard let _ = app else {
            let genericError = NSError(domain: "AppHandler", code: -2, userInfo: [NSLocalizedDescriptionKey: "Application failed to launch"])
            completion(false, genericError)
            return
        }
        
        completion(true, nil)
    }
}

}

// C-compatible function to get installed apps as JSON
@_cdecl("get_installed_apps_swift")
public func getInstalledAppsSwift() -> UnsafeMutablePointer<CChar>? {
    let encoder = JSONEncoder()
    
    do {
        let apps = AppHandler.getInstalledApps()
        let jsonData = try encoder.encode(apps)
        
        if let jsonString = String(data: jsonData, encoding: .utf8) {
            return strdup(jsonString)
        }
    } catch {
        print("Error encoding apps: \(error)")
    }
    
    return nil
}

// C-compatible function to get running apps as JSON
@_cdecl("get_running_apps_swift")
public func getRunningAppsSwift() -> UnsafeMutablePointer<CChar>? {
    let encoder = JSONEncoder()
    
    do {
        let apps = AppHandler.getRunningApps()
        let jsonData = try encoder.encode(apps)
        
        if let jsonString = String(data: jsonData, encoding: .utf8) {
            return strdup(jsonString)
        }
    } catch {
        print("Error encoding apps: \(error)")
    }
    
    return nil
}

// C-compatible function to get app icon
@_cdecl("get_app_icon_swift")
public func getAppIconSwift(path: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>? {
    guard let path = path,
          let pathString = String(cString: path, encoding: .utf8) else {
        return nil
    }
    
    if let icon = AppHandler.getAppIcon(path: pathString) {
        return strdup(icon)
    }
    
    return nil
}

// C-compatible function to switch to an app
@_cdecl("switch_to_app_swift")
public func switchToAppSwift(pid: Int32) -> Bool {
    return AppHandler.switchToApp(pid: pid)
}

// C-compatible function to force quit an app
@_cdecl("force_quit_app_swift")
public func forceQuitAppSwift(pid: Int32) -> Bool {
    return AppHandler.forceQuitApplication(pid: pid)
}

// C-compatible function to restart an app
@_cdecl("restart_app_swift")
public func restartAppSwift(path: UnsafePointer<CChar>?) -> Bool {
    guard let path = path,
          let pathString = String(cString: path, encoding: .utf8) else {
        return false
    }
    
    var result = false
    let semaphore = DispatchSemaphore(value: 0)
    
    AppHandler.restartApplication(path: pathString) { success, error in
        result = success
        if let error = error {
            print("Restart error: \(error)")
        }
        semaphore.signal()
    }
    
    // Wait for 10 seconds maximum
    _ = semaphore.wait(timeout: .now() + 10)
    
    return result
}
