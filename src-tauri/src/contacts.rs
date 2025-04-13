// the objc crate has some cgs errors that it throws
// these shouldn't impact functionality so silencing for now until they fix them in a new version or something
#![allow(unexpected_cfgs)]
use core_foundation::{
    base::TCFType,
    string::{CFString, CFStringRef},
};
use objc::{
    class, msg_send,
    runtime::{Class, Object, BOOL, NO, YES},
    sel, sel_impl,
};
use std::ffi::{c_char, CStr, CString};
use std::os::raw::c_int;
use std::{process::Command, str};
use thiserror::Error;

use block2::RcBlock;
use objc2_contacts::{CNAuthorizationStatus, CNContactStore, CNEntityType};
use objc2_foundation::NSError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    pub id: String,
    pub name: Option<String>,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    pub phone_numbers: Vec<ContactPhone>,
    pub image_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactEmail {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactPhone {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactAddress {
    pub label: String,
    pub street: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
}

#[derive(Debug, Error)]
pub enum ContactError {
    #[error("Permission denied to access contacts")]
    PermissionDenied,

    #[error("Failed to access contacts: {0}")]
    AccessError(String),

    #[error("Unexpected null value")]
    NullValue,

    #[error("JSON parsing error: {0}")]
    JsonError(String),
}

extern "C" {
    fn check_contacts_permission_swift() -> c_int;
    fn request_contacts_permission_swift() -> c_int;
    fn fetch_contacts_swift() -> *mut c_char;
    fn free_string_swift(pointer: *mut c_char);
}

//TODO remove the permissions and check the flow

// Check if app has permission to access contacts
pub fn check_contacts_permission() -> Result<bool, ContactError> {
    // CNAuthorizationStatus: NotDetermined = 0, Restricted = 1, Denied = 2, Authorized = 3
    println!("checking permissions...");

    let status = unsafe { check_contacts_permission_swift() };

    println!("permissions: {:?}", status);
    match status {
        3 => Ok(true),  // Authorized
        _ => Ok(false), // Not authorized
    }
}

// Request permission to access contacts
pub fn request_contacts_permission() -> Result<bool, ContactError> {
    let status = unsafe { request_contacts_permission_swift() };
    match status {
        3 => Ok(true),  // Authorized
        _ => Ok(false), // Not authorized
    }
}

// Fetch contacts using the Swift bridge
pub fn get_contacts() -> Result<Vec<BasicContact>, ContactError> {
    println!("getting contacts...");
    // First check permission
    if !check_contacts_permission()? {
        // Try to request permission
        if !request_contacts_permission()? {
            return Err(ContactError::PermissionDenied);
        }
    }

    // Call Swift function to fetch contacts
    let contacts_json_ptr = unsafe { fetch_contacts_swift() };

    println!("contacts:{:?}", &contacts_json_ptr);

    // Check if pointer is null
    if contacts_json_ptr.is_null() {
        return Err(ContactError::NullValue);
    }

    // Convert C string to Rust string
    let contacts_json = unsafe {
        let c_str = CStr::from_ptr(contacts_json_ptr);
        let result = c_str
            .to_str()
            .map_err(|e| ContactError::AccessError(format!("Invalid UTF-8: {}", e)))?
            .to_owned();

        // Free the C string
        free_string_swift(contacts_json_ptr);

        result
    };

    println!("contact json:{:?}", &contacts_json);

    // Check if we got an error response
    if contacts_json.starts_with("{\"error\":") {
        let error_msg = contacts_json
            .replace("{\"error\":", "")
            .replace("\"}", "")
            .replace("\"", "");
        if error_msg.contains("not_authorized") {
            return Err(ContactError::PermissionDenied);
        }
        return Err(ContactError::AccessError(error_msg));
    }

    // Parse JSON
    let contacts: Vec<BasicContact> = serde_json::from_str(&contacts_json)
        .map_err(|e| ContactError::JsonError(format!("Failed to parse contacts JSON: {}", e)))?;

    Ok(contacts)
}

// Tauri command
#[tauri::command]
pub async fn get_contacts_command() -> Result<Vec<BasicContact>, String> {
    match get_contacts() {
        Ok(contacts) => Ok(contacts),
        Err(err) => Err(err.to_string()),
    }
}

// /// Uses AppleScript (run via `osascript`) to retrieve contacts.
// /// It returns a JSON-encoded list of `Contact` objects, which we then parse.
// pub fn get_contacts_via_osascript() -> Result<Vec<BasicContact>, ContactError> {
//     // This AppleScript:
//     // 1. Tells the "Contacts" app to get every person.
//     // 2. Builds a JSON string: [{ "id": "...", "name": "...", "given_name": "...",
//     //    "family_name": "...", "phone_numbers": [...], "image_available": true/false }, ...]
//     // 3. Returns that JSON string.
//     //
//     // NOTE: For phone numbers, we set `label = "Phone"` by default in this example,
//     //       and we just fetch the raw phone value. If you want more elaborate label logic,
//     //       you can expand the AppleScript.

//     let applescript = r#"
// tell application "Contacts"
//     set allPeople to every person
//     set totalCount to count of allPeople

//     set resultJSON to "["
//     repeat with i from 1 to totalCount
//         set p to item i of allPeople

//         -- Use property references without 'of' syntax
//         try
//             set theID to id of p
//         on error
//             set theID to ""
//         end try

//         -- Build a minimal JSON object
//         set contactJSON to "{"
//         set contactJSON to contactJSON & "\"id\":\"" & theID & "\","
//         set contactJSON to contactJSON & "}"

//         if i < totalCount then
//             set contactJSON to contactJSON & ","
//         end if

//         set resultJSON to resultJSON & contactJSON
//     end repeat

//     set resultJSON to resultJSON & "]"
//     return resultJSON
// end tell
// "#;

//     // 1) Run the script via `osascript`
//     let output = Command::new("osascript")
//         .arg("-e")
//         .arg(applescript)
//         .output()
//         .map_err(|e| ContactError::ScriptError(format!("Failed to run osascript: {e}")))?;

//     // 2) If the script failed (e.g. user denied permission), handle error
//     if !output.status.success() {
//         let stderr = String::from_utf8_lossy(&output.stderr).to_string();
//         if stderr.contains("not authorized") || stderr.contains("denied") {
//             return Err(ContactError::PermissionDenied);
//         }
//         return Err(ContactError::ScriptError(stderr));
//     }

//     // 3) Convert stdout to a Rust string
//     let stdout = String::from_utf8_lossy(&output.stdout).to_string();
//     if stdout.trim().is_empty() {
//         // Possibly user canceled the permissions dialog
//         return Err(ContactError::PermissionDenied);
//     }

//     // 4) Parse the minimal JSON into a Vec<BasicContact>
//     let contacts: Vec<BasicContact> = serde_json::from_str(&stdout).map_err(|e| {
//         ContactError::ScriptError(format!("JSON parse error: {e}\nOutput was: {stdout}"))
//     })?;

//     println!("the contacts: {:?}", contacts);

//     Ok(contacts)
// }

// // Example Tauri command
// #[tauri::command]
// pub async fn get_contacts_command() -> Result<Vec<BasicContact>, String> {
//     match get_contacts_via_osascript() {
//         Ok(list) => Ok(list),
//         Err(err) => Err(err.to_string()),
//     }
// }

// pub fn check_contacts_permission() -> Result<bool, ContactError> {
//     // NotDetermined = 0,
//     // Restricted = 1,
//     // Denied = 2,
//     // Authorized = 3,
//     // Limited = 4

//     unsafe {
//         let contacts_class: &Class = class!(CNContactStore);

//         // Get current authorization status
//         let entity_type = 0; // CNEntityTypeContacts = 0
//         let auth_status: i32 =
//             msg_send![contacts_class, authorizationStatusForEntityType:entity_type];
//         println!("checking auth: {}", auth_status);
//         match auth_status {
//             3 => Ok(true), // CNAuthorizationStatusAuthorized
//             _ => Ok(false),
//         }
//     }
// }

// fn fetch_contacts() -> Result<Vec<Contact>, Box<dyn std::error::Error>> {
//     unsafe {
//         let store = CNContactStore::new();

//         let entity_type = CNEntityType::Contacts;
//         // Check authorization status (important!)
//         let auth_status = CNContactStore::authorizationStatusForEntityType(entity_type);
//         println!("Contacts Auth Status: {:?}", auth_status);
//         //Handle cases where access is denied or restricted
//         if auth_status == CNAuthorizationStatus::NotDetermined {
//             // We do NOT block. We'll handle the user’s response in a callback.

//             let callback = move |granted: bool, _error: *mut NSError| {
//                 // In real code, you'd jump back to main thread or queue for UI.
//                 on_contacts_permission_result(granted);
//             };

//             // Convert closure to an RcBlock recognized by Objective-C
//             let completion_block = RcBlock::new(callback);

//             // Pass `&*completion_block` to the ObjC method
//             store.requestAccessForEntityType_completionHandler(entity_type, &*completion_block);

//             // The function returns immediately; user sees a system dialog,
//             // and later `on_contacts_permission_result()` is called.
//         } else if auth_status == CNAuthorizationStatus::Authorized {
//             // Already authorized
//             // on_contacts_permission_result(true);
//             println!("yay we got it!!!");
//         } else {
//             println!(" we didn't got it!!!");
//             // Previously denied/restricted
//             // on_contacts_permission_result(false);
//         }

//     }
//              let contacts: Vec<Contact> = Vec::new();
//              Ok(contacts)

// }

// if auth_status == CNAuthorizationStatus::NotDetermined {
//     // Create shared state to receive the result
//     let contact_permission_granted = Arc::new(Mutex::new(false));
//     let contact_permission_processed = Arc::new(Mutex::new(false));

//     // Clone for use in the block
//     let contact_permission_granted_clone = Arc::clone(&contact_permission_granted);
//     let contact_permission_processed_clone = Arc::clone(&contact_permission_processed);

//     // Create the completion handler block
//     let completion_block = RcBlock::new(move |granted: bool, _error: *mut NSError| {
//         *granted_clone.lock().unwrap() = granted;
//         *processed_clone.lock().unwrap() = true;
//     });

//     store.requestAccessForEntityType_completionHandler(entity_type, &*completion_block);

//         if auth_status == CNAuthorizationStatus::NotDetermined {
//             // We do NOT block. We'll handle the user’s response in a callback.

//             let callback = move |granted: bool, _error: *mut NSError| {
//                 // In real code, you'd jump back to main thread or queue for UI.
//                 on_contacts_permission_result(granted);
//             };

//             // Convert closure to an RcBlock recognized by Objective-C
//             let completion_block = RcBlock::new(callback);

//             // Pass `&*completion_block` to the ObjC method
//             store.requestAccessForEntityType_completionHandler(entity_type, &*completion_block);

//             // The function returns immediately; user sees a system dialog,
//             // and later `on_contacts_permission_result()` is called.
//         } else if auth_status == CNAuthorizationStatus::Authorized {
//             // Already authorized
//             on_contacts_permission_result(true);
//         } else {
//             // Previously denied/restricted
//             on_contacts_permission_result(false);
//         }
//     }
// }

// Wait for the callback to complete (with timeout)
// let start_time = std::time::Instant::now();
// let timeout = Duration::from_secs(10);

// while !*contact_permission_processed.lock().unwrap() {
//     if start_time.elapsed() > timeout {
//         return Err("Timeout waiting for contacts permission".into());
//     }
//     thread::sleep(Duration::from_millis(100));
// }

// // Check if permission was granted
// if !*contact_permission_granted.lock().unwrap() {
//     return Err("Permission to access contacts was denied".into());
// }
// } else if auth_status != CNAuthorizationStatus::Authorized {
//     // Permission was previously denied or restricted
//     return Err("Permission to access contacts was denied or restricted".into());
// }

//TODO: figure out the completion handler here

// let contacts: Vec<Contact> = Vec::new();
// let all_containers = store.all_containers()?; // Or default container
// let predicate = objc2_contacts::CNCont contact::predicate::Contact::all(); // Fetch all
// let keys_to_fetch = vec![
//     objc2_contacts::contact::keys::GivenName,
//     objc2_contacts::contact::keys::FamilyName,
//     objc2_contacts::contact::keys::EmailAddresses,
//     // Add other keys you need
// ];

// let contacts = store.unifiedContactsMatchingPredicate_keysToFetch_error(predicate, keys_to_fetch)?;

// // Process the Vec<Contact>
// for contact in &contacts {
//     // Use methods provided by the crate's Contact type
//     // e.g., contact.given_name(), contact.email_addresses()
//     println!(
//         "Found: {:?} {:?}",
//         contact.given_name(),
//         contact.family_name()
//     );
// }
// Ok(contacts)

//         Ok(contacts)
//     }
// }

// fn on_contacts_permission_result(granted: bool) {
//     if granted {
//         println!("Contacts permission granted! Now fetch your contacts.");
//         // fetch_and_display_contacts();
//     } else {
//         println!("Contacts permission denied or restricted!");
//         // Possibly show an alert or disable a feature
//     }
// }

// pub fn request_contacts_permission() -> Result<bool, ContactError> {
//     unsafe {
//         let contacts_class = class!(CNContactStore);

//         // Get current authorization status
//         let entity_type = 0; // CNEntityTypeContacts = 0
//         let auth_status: i32 =
//             msg_send![contact_store, authorizationStatusForEntityType: entity_type];

//         match auth_status {
//             0 => {
//                 // CNAuthorizationStatusNotDetermined
//                 // Request access
//                 let completion_handler = Box::new(|granted: BOOL, error: *mut Object| {
//                     // This callback would be handled by Objective-C runtime
//                 });
//                 let _: () = msg_send![contact_store, requestAccessForEntityType:entity_type
//                                       completionHandler:completion_handler];

//                 // After request, check status again
//                 let new_status: i32 =
//                     msg_send![contact_store, authorizationStatusForEntityType: entity_type];
//                 Ok(new_status == 3) // CNAuthorizationStatusAuthorized
//             }
//             3 => Ok(true),  // CNAuthorizationStatusAuthorized
//             _ => Ok(false), // Denied or Restricted
//         }
//     }
// }

// pub fn get_contacts() -> Result<Vec<Contact>, ContactError> {
//     unsafe {
//         // First check permission
//         if !check_contacts_permission()? {
//             return Err(ContactError::PermissionDenied);
//         }

//         let contacts_class = class!(CNContactStore);
//         let contact_store: *mut Object = msg_send![contacts_class, alloc];
//         let contact_store: *mut Object = msg_send![contact_store, init];

//         // Create the key descriptors for the properties we want
//         let key_class = class!(CNContactFetchRequest);
//         let keys_to_fetch = create_contact_keys();

//         let fetch_request: *mut Object = msg_send![key_class, alloc];
//         let fetch_request: *mut Object =
//             msg_send![fetch_request, initWithKeysToFetch:keys_to_fetch];

//         let mut contacts = Vec::new();
//         let mut fetch_error: *mut Object = std::ptr::null_mut();

//         // Enumerate contacts
//         let success: BOOL = msg_send![
//             contact_store,
//             enumerateContactsWithFetchRequest:fetch_request
//             error:&mut fetch_error
//             usingBlock:&mut |contact: *mut Object, stop_ptr: *mut BOOL| {
//                 // Process each contact
//                 if let Ok(rust_contact) = process_contact(contact) {
//                     contacts.push(rust_contact);
//                 }
//             }
//         ];

//         if success == NO {
//             if fetch_error.is_null() {
//                 return Err(ContactError::AccessError(
//                     "Unknown error fetching contacts".into(),
//                 ));
//             } else {
//                 let description: *mut Object = msg_send![fetch_error, localizedDescription];
//                 let error_str = nsstring_to_string(description);
//                 return Err(ContactError::AccessError(error_str));
//             }
//         }

//         Ok(contacts)
//     }
// }

// unsafe fn create_contact_keys() -> *mut Object {
//     // Create an array of keys we want to fetch
//     let array_class = class!(NSMutableArray);
//     let keys_array: *mut Object = msg_send![array_class, alloc];
//     let keys_array: *mut Object = msg_send![keys_array, init];

//     // Add the keys we want
//     add_key_to_array(keys_array, "CNContactIdentifierKey");
//     add_key_to_array(keys_array, "CNContactGivenNameKey");
//     add_key_to_array(keys_array, "CNContactFamilyNameKey");
//     add_key_to_array(keys_array, "CNContactPhoneNumbersKey");
//     add_key_to_array(keys_array, "CNContactImageDataAvailableKey");

//     keys_array
// }

// unsafe fn add_key_to_array(array: *mut Object, key: &str) {
//     let contact_class = class!(CNContact);
//     let key_obj: *mut Object = msg_send![contact_class, performSelector:sel!(valueForKey:)
//                                          withObject:nsstring_from_str(key)];
//     let _: () = msg_send![array, addObject:key_obj];
// }

// unsafe fn process_contact(contact: *mut Object) -> Result<Contact, ContactError> {
//     // Extract identifier
//     let identifier: *mut Object = msg_send![contact, identifier];
//     let id = nsstring_to_string(identifier);

//     // Extract names
//     let given_name_obj: *mut Object = msg_send![contact, givenName];
//     let family_name_obj: *mut Object = msg_send![contact, familyName];

//     let given_name = if !given_name_obj.is_null() {
//         Some(nsstring_to_string(given_name_obj))
//     } else {
//         None
//     };

//     let family_name = if !family_name_obj.is_null() {
//         Some(nsstring_to_string(family_name_obj))
//     } else {
//         None
//     };

//     // Full name
//     let name = match (&given_name, &family_name) {
//         (Some(g), Some(f)) => Some(format!("{} {}", g, f)),
//         (Some(g), None) => Some(g.clone()),
//         (None, Some(f)) => Some(f.clone()),
//         (None, None) => None,
//     };

//     // Image availability
//     let image_available: BOOL = msg_send![contact, imageDataAvailable];

//     let phone_numbers = extract_phones(contact)?;

//     Ok(Contact {
//         id,
//         name,
//         given_name,
//         family_name,
//         phone_numbers,
//         image_available: image_available == YES,
//     })
// }

// // Helper functions for string conversions
// unsafe fn nsstring_from_str(s: &str) -> *mut Object {
//     let cls = class!(NSString);
//     let s = CFString::new(s);
//     let s: CFStringRef = s.as_concrete_TypeRef();
//     msg_send![cls, stringWithCFString:s]
// }

// unsafe fn nsstring_to_string(nsstring: *mut Object) -> String {
//     if nsstring.is_null() {
//         return String::new();
//     }

//     let utf8_encoding = 4_u64; // NSUTF8StringEncoding
//     let nsstring: *mut Object = msg_send![nsstring, retain];
//     let cstr: *const i8 = msg_send![nsstring, UTF8String];
//     let len: usize = msg_send![nsstring, lengthOfBytesUsingEncoding:utf8_encoding];

//     let bytes = std::slice::from_raw_parts(cstr as *const u8, len);
//     let string = String::from_utf8_lossy(bytes).into_owned();

//     let _: () = msg_send![nsstring, release];
//     string
// }

// unsafe fn extract_phones(contact: *mut Object) -> Result<Vec<ContactPhone>, ContactError> {
//     // Implementation for extracting phone numbers
//     let mut result = Vec::new();

//     // Get the phone numbers array
//     let phones: *mut Object = msg_send![contact, phoneNumbers];
//     let count: usize = msg_send![phones, count];

//     for i in 0..count {
//         let labeled_value: *mut Object = msg_send![phones, objectAtIndex:i];
//         let label: *mut Object = msg_send![labeled_value, label];
//         let value: *mut Object = msg_send![labeled_value, value];

//         let label_str = if !label.is_null() {
//             let localized: *mut Object =
//                 msg_send![class!(CNLabeledValue), localizedStringForLabel:label];
//             nsstring_to_string(localized)
//         } else {
//             "Phone".to_string()
//         };

//         // Get string value from phone number
//         let string_value: *mut Object = msg_send![value, stringValue];
//         let value_str = nsstring_to_string(string_value);

//         result.push(ContactPhone {
//             label: label_str,
//             value: value_str,
//         });
//     }

//     Ok(result)
// }

// #[tauri::command]
// pub async fn check_contacts_permission_command() -> Result<bool, String> {
//     check_contacts_permission().map_err(|e| e.to_string())
// }

// // #[tauri::command]
// // pub async fn request_contacts_permission_command() -> Result<bool, String> {
// //     request_contacts_permission().map_err(|e| e.to_string())
// // }

// #[tauri::command]
// pub async fn get_contacts_command() -> Result<Vec<Contact>, String> {
//     fetch_contacts().map_err(|e| e.to_string())
// }
// impl Contact {
//     /// Get all contacts from macOS Contacts app
//     pub async fn get_contacts() -> Result<Vec<Contact>, ContactError> {
//         // Create a temporary AppleScript file to access contacts
//         let script_path = Self::create_contacts_script()?;

//         let output = Self::run_applescript(&script_path).await?;

//         let contacts: Vec<Contact> = match serde_json::from_str(&output) {
//             Ok(contacts) => contacts,
//             Err(err) => {
//                 return Err(ContactError::ParseError(err.to_string()));
//             }
//         };

//         println!("the contacts in the backend: {:?}", contacts);
//         // clean up temp file
//         let _ = fs::remove_file(script_path);

//         Ok(contacts)
//     }

//     /// Get a contact image by ID
//     pub async fn get_contact_image(contact_id: &str) -> Result<Vec<u8>, ContactError> {
//         let script = format!(
//             r#"
//         osascript -e '
//         use framework "Foundation"
//         use framework "Contacts"
//         use scripting additions

//         set contactId to "{}"

//         set contactStore to current application's CNContactStore's alloc()'s init()
//         set keysToFetch to current application's NSArray's arrayWithObject:(current application's CNContactImageDataKey)

//         try
//             set predicate to current application's CNContact's predicateForContactsWithIdentifiers:(current application's NSArray's arrayWithObject:(contactId))
//             set contacts to contactStore's unifiedContactsMatchingPredicate:predicate keysToFetch:keysToFetch error:(reference)

//             if (count of contacts) > 0 then
//                 set contact to item 1 of contacts
//                 if contact's imageData() is not missing value then
//                     set imageData to contact's imageData()
//                     set base64String to (current application's NSString's alloc()'s initWithData:imageData encoding:(current application's NSUTF8StringEncoding))
//                     return base64String as text
//                 end if
//             end if

//             return ""
//         on error errMsg
//             return ""
//         end try
//         '
//         "#,
//             contact_id
//         );

//         let output = Command::new("sh").arg("-c").arg(&script).output().await?;

//         if !output.status.success() {
//             return Err(ContactError::Script(
//                 String::from_utf8_lossy(&output.stderr).to_string(),
//             ));
//         }

//         let base64_data = String::from_utf8_lossy(&output.stdout).trim().to_string();
//         if base64_data.is_empty() {
//             return Err(ContactError::ParseError("No image available".to_string()));
//         }

//         // Decode the base64 string to bytes
//         let image_data = base64::decode(base64_data)
//             .map_err(|e| ContactError::ParseError(format!("Failed to decode image: {}", e)))?;

//         Ok(image_data)
//     }

//     /// Creates a temporary AppleScript file to fetch contacts
//     fn create_contacts_script() -> Result<PathBuf, ContactError> {
//         let temp_dir = std::env::temp_dir();
//         let script_path = temp_dir.join("fetch_contacts.scpt");

//         let script = r#"
//         use framework "Foundation"
//         use framework "Contacts"
//         use scripting additions

//         -- Create a contact store
//         set contactStore to current application's CNContactStore's alloc()'s init()

//         -- Define the contact keys we want to fetch
//         set keysToFetch to current application's NSArray's arrayWithObjects_count_({current application's CNContactIdentifierKey, current application's CNContactGivenNameKey, current application's CNContactFamilyNameKey, current application's CNContactEmailAddressesKey, current application's CNContactPhoneNumbersKey, current application's CNContactPostalAddressesKey, current application's CNContactBirthdayKey, current application's CNContactImageDataAvailableKey}, 11)

//         -- Request permission to access contacts
//         set authStatus to contactStore's authorizationStatusForEntityType:(current application's CNEntityTypeContacts)

//         if authStatus is not equal to (current application's CNAuthorizationStatusAuthorized) then
//             contactStore's requestAccessForEntityType:current application's CNEntityTypeContacts completionHandler:(missing value)

//             set authStatus to contactStore's authorizationStatusForEntityType:(current application's CNEntityTypeContacts)
//             if authStatus is not equal to (current application's CNAuthorizationStatusAuthorized) then
//                 return "{\"error\": \"Permission denied to access contacts\"}"
//             end if
//         end if

//         -- Fetch all contacts
//         set fetchRequest to current application's CNContactFetchRequest's alloc()'s initWithKeysToFetch:keysToFetch

//         set contactsArray to current application's NSMutableArray's alloc()'s init()

//         try
//             contactStore's enumerateContactsWithFetchRequest:fetchRequest error:(reference) usingBlock:(handler |contact| stop |
//                 set contactDict to current application's NSMutableDictionary's alloc()'s init()

//                 -- Basic info
//                 contactDict's setValue:contact's identifier() forKey:"id"
//                 if contact's givenName() is not missing value then
//                     contactDict's setValue:contact's givenName() forKey:"given_name"
//                 end if
//                 if contact's familyName() is not missing value then
//                     contactDict's setValue:contact's familyName() forKey:"family_name"
//                 end if

//                 -- Full name
//                 set fullName to ""
//                 if contact's givenName() is not missing value and contact's familyName() is not missing value then
//                     set fullName to contact's givenName() & " " & contact's familyName()
//                 else if contact's givenName() is not missing value then
//                     set fullName to contact's givenName()
//                 else if contact's familyName() is not missing value then
//                     set fullName to contact's familyName()
//                 end if
//                 if fullName is not equal to "" then
//                     contactDict's setValue:fullName forKey:"name"
//                 end if

//                 -- Organization and job
//                 if contact's organizationName() is not missing value then
//                     contactDict's setValue:contact's organizationName() forKey:"organization"
//                 end if
//                 if contact's jobTitle() is not missing value then
//                     contactDict's setValue:contact's jobTitle() forKey:"job_title"
//                 end if

//                 -- Notes
//                 if contact's note() is not missing value then
//                     contactDict's setValue:contact's note() forKey:"notes"
//                 end if

//                 -- Image availability
//                 contactDict's setValue:contact's imageDataAvailable() forKey:"image_available"

//                 -- Birthday
//                 if contact's birthday() is not missing value then
//                     set birthdayComponents to contact's birthday()
//                     set year to birthdayComponents's |year|()
//                     set month to birthdayComponents's |month|()
//                     set day to birthdayComponents's |day|()
//                     set birthdayString to year as text & "-" & (month as text) & "-" & (day as text)
//                     contactDict's setValue:birthdayString forKey:"birthday"
//                 end if

//                 -- Email addresses
//                 set emailsArray to current application's NSMutableArray's alloc()'s init()
//                 repeat with emailAddress in contact's emailAddresses()
//                     set emailDict to current application's NSMutableDictionary's alloc()'s init()

//                     set label to emailAddress's label()
//                     if label is not missing value then
//                         set localizedLabel to current application's CNLabeledValue's localizedStringForLabel:label
//                         emailDict's setValue:localizedLabel forKey:"label"
//                     else
//                         emailDict's setValue:"Email" forKey:"label"
//                     end if

//                     emailDict's setValue:emailAddress's value() as text forKey:"value"
//                     emailsArray's addObject:emailDict
//                 end repeat
//                 contactDict's setValue:emailsArray forKey:"emails"

//                 -- Phone numbers
//                 set phonesArray to current application's NSMutableArray's alloc()'s init()
//                 repeat with phoneNumber in contact's phoneNumbers()
//                     set phoneDict to current application's NSMutableDictionary's alloc()'s init()

//                     set label to phoneNumber's label()
//                     if label is not missing value then
//                         set localizedLabel to current application's CNLabeledValue's localizedStringForLabel:label
//                         phoneDict's setValue:localizedLabel forKey:"label"
//                     else
//                         phoneDict's setValue:"Phone" forKey:"label"
//                     end if

//                     set formattedNumber to (phoneNumber's value()'s stringValue()) as text
//                     phoneDict's setValue:formattedNumber forKey:"value"
//                     phonesArray's addObject:phoneDict
//                 end repeat
//                 contactDict's setValue:phonesArray forKey:"phone_numbers"

//                 -- Postal addresses
//                 set addressesArray to current application's NSMutableArray's alloc()'s init()
//                 repeat with postalAddress in contact's postalAddresses()
//                     set addressDict to current application's NSMutableDictionary's alloc()'s init()

//                     set label to postalAddress's label()
//                     if label is not missing value then
//                         set localizedLabel to current application's CNLabeledValue's localizedStringForLabel:label
//                         addressDict's setValue:localizedLabel forKey:"label"
//                     else
//                         addressDict's setValue:"Address" forKey:"label"
//                     end if

//                     set address to postalAddress's value()

//                     if address's street() is not missing value then
//                         addressDict's setValue:address's street() as text forKey:"street"
//                     end if
//                     if address's city() is not missing value then
//                         addressDict's setValue:address's city() as text forKey:"city"
//                     end if
//                     if address's state() is not missing value then
//                         addressDict's setValue:address's state() as text forKey:"state"
//                     end if
//                     if address's postalCode() is not missing value then
//                         addressDict's setValue:address's postalCode() as text forKey:"postal_code"
//                     end if
//                     if address's country() is not missing value then
//                         addressDict's setValue:address's country() as text forKey:"country"
//                     end if

//                     addressesArray's addObject:addressDict
//                 end repeat
//                 contactDict's setValue:addressesArray forKey:"addresses"

//                 contactsArray's addObject:contactDict
//             end)

//             set jsonData to current application's NSJSONSerialization's dataWithJSONObject:contactsArray options:0 |error|:(reference)
//             set jsonString to (current application's NSString's alloc()'s initWithData:jsonData encoding:(current application's NSUTF8StringEncoding)) as text

//             return jsonString
//         on error errMsg
//             return "{\"error\": \"" & errMsg & "\"}"
//         end try
//         "#;

//         fs::write(&script_path, script)?;
//         Ok(script_path)
//     }

//     /// Runs the AppleScript and returns the output
//     async fn run_applescript(script_path: &Path) -> Result<String, ContactError> {
//         // Run the script with a timeout to prevent hanging
//         let script_cmd = format!("osascript '{}'", script_path.display());

//         let run_script = async {
//             let output = Command::new("sh")
//                 .arg("-c")
//                 .arg(&script_cmd)
//                 .output()
//                 .await?;

//             if !output.status.success() {
//                 return Err(ContactError::Script(
//                     String::from_utf8_lossy(&output.stderr).to_string(),
//                 ));
//             }

//             let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

//             // Check for permission error
//             if stdout.contains("Permission denied") {
//                 return Err(ContactError::PermissionDenied);
//             }

//             // Check for JSON parsing
//             if let Ok(error_obj) = serde_json::from_str::<serde_json::Value>(&stdout) {
//                 if let Some(error_msg) = error_obj.get("error").and_then(|e| e.as_str()) {
//                     return Err(ContactError::Script(error_msg.to_string()));
//                 }
//             }

//             Ok(stdout)
//         };

//         // Add a timeout to prevent hanging if permissions dialog appears
//         match timeout(Duration::from_secs(30), run_script).await {
//             Ok(result) => result,
//             Err(_) => Err(ContactError::Timeout),
//         }
//     }
// }

// #[tauri::command]
// pub async fn get_contacts() -> Result<Vec<Contact>, String> {
//     Contact::get_contacts().await.map_err(|e| e.to_string())
// }
