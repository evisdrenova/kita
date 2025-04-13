import Foundation
import Contacts

@_cdecl("check_contacts_permission_swift")
public func checkContactsPermission() -> Int32 {
    let status = CNContactStore.authorizationStatus(for: .contacts)
    return Int32(status.rawValue)
}

@_cdecl("request_contacts_permission_swift")
public func requestContactsPermission() -> Int32 {
    let semaphore = DispatchSemaphore(value: 0)
    var result: Int32 = 0
    
    let store = CNContactStore()
    store.requestAccess(for: .contacts) { (granted, error) in
        if granted {
            result = 3 // Authorized
        }
        semaphore.signal()
    }
    
    // Wait with a timeout
    _ = semaphore.wait(timeout: .now() + 30)
    return result
}

struct ContactPhone: Codable {
    var label: String
    var value: String
}

struct BasicContact: Codable {
    var id: String
    var given_name: String?
    var family_name: String?
    var phone_numbers: [ContactPhone]?
    var image_available: Bool
}

@_cdecl("fetch_contacts_swift")
public func fetchContacts() -> UnsafeMutablePointer<CChar>? {
    let status = CNContactStore.authorizationStatus(for: .contacts)
    if status != .authorized {
        let errorJson = "{\"error\": \"not_authorized\"}"
        let errorCString = strdup(errorJson)
        return errorCString
    }
    
    let store = CNContactStore()
    let keysToFetch: [CNKeyDescriptor] = [
        CNContactIdentifierKey as CNKeyDescriptor,
        CNContactGivenNameKey as CNKeyDescriptor,
        CNContactFamilyNameKey as CNKeyDescriptor,
        CNContactPhoneNumbersKey as CNKeyDescriptor,
        CNContactImageDataAvailableKey as CNKeyDescriptor
    ]
    
    var contacts: [BasicContact] = []
    
    do {
        let request = CNContactFetchRequest(keysToFetch: keysToFetch)
        try store.enumerateContacts(with: request) { (contact, _) in
            let phoneNumbers = contact.phoneNumbers.map { 
                ContactPhone(
                    label: CNLabeledValue<CNPhoneNumber>.localizedString(forLabel: $0.label ?? ""),
                    value: $0.value.stringValue
                )
            }
            
            let basicContact = BasicContact(
                id: contact.identifier,
                given_name: contact.givenName.isEmpty ? nil : contact.givenName,
                family_name: contact.familyName.isEmpty ? nil : contact.familyName,
                phone_numbers: phoneNumbers.isEmpty ? nil : phoneNumbers,
                image_available: contact.imageDataAvailable
            )
            
            contacts.append(basicContact)
        }
        
        let encoder = JSONEncoder()
        let jsonData = try encoder.encode(contacts)
        if let jsonString = String(data: jsonData, encoding: .utf8) {
            let cString = strdup(jsonString)
            return cString
        }
    } catch {
        let errorJson = "{\"error\": \"\(error.localizedDescription)\"}"
        let errorCString = strdup(errorJson)
        return errorCString
    }
    
    return nil
}

@_cdecl("free_string_swift")
public func freeString(pointer: UnsafeMutablePointer<CChar>?) {
    if let pointer = pointer {
        free(pointer)
    }
}