use serde::{Deserialize, Serialize};
use std::ffi::{c_char, CStr};
use std::os::raw::c_int;
use std::str;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    pub id: String,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    pub phone_numbers: Option<Vec<ContactPhone>>,
    pub image_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactPhone {
    pub label: String,
    pub value: String,
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

pub fn check_contacts_permission() -> Result<bool, ContactError> {
    // CNAuthorizationStatus: NotDetermined = 0, Restricted = 1, Denied = 2, Authorized = 3
    let status = unsafe { check_contacts_permission_swift() };
    match status {
        3 => Ok(true),  // Authorized
        _ => Ok(false), // Not authorized
    }
}

pub fn request_contacts_permission() -> Result<bool, ContactError> {
    let status = unsafe { request_contacts_permission_swift() };
    match status {
        3 => Ok(true),  // Authorized
        _ => Ok(false), // Not authorized
    }
}

pub fn get_contacts() -> Result<Vec<Contact>, ContactError> {
    println!("getting contacts...");
    if !check_contacts_permission()? {
        if !request_contacts_permission()? {
            return Err(ContactError::PermissionDenied);
        }
    }

    let contacts_json_ptr = unsafe { fetch_contacts_swift() };

    // Check if pointer is null
    if contacts_json_ptr.is_null() {
        return Err(ContactError::NullValue);
    }

    // Convert C string to Rust string
    let contacts_json = unsafe {
        let c_str = CStr::from_ptr(contacts_json_ptr);
        let result = c_str
            .to_str()
            .map_err(|e| ContactError::JsonError(format!("Invalid UTF-8: {}", e)))?
            .to_owned();

        // Free the C string
        free_string_swift(contacts_json_ptr);

        result
    };

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

    let contacts: Vec<Contact> = serde_json::from_str(&contacts_json)
        .map_err(|e| ContactError::JsonError(format!("Failed to parse contacts JSON: {}", e)))?;

    Ok(contacts)
}

#[tauri::command]
pub async fn get_contacts_command() -> Result<Vec<Contact>, String> {
    match get_contacts() {
        Ok(contacts) => Ok(contacts),
        Err(ContactError::PermissionDenied) => {
            Err("Permission denied to access contacts".to_string())
        }
        Err(ContactError::AccessError(msg)) => Err(format!("Access error: {}", msg)),
        Err(err) => Err(err.to_string()),
    }
}
