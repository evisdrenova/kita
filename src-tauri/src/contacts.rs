use dirs;
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    pub id: String,
    pub name: Option<String>,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    pub emails: Vec<ContactEmail>,
    pub phone_numbers: Vec<ContactPhone>,
    pub addresses: Vec<ContactAddress>,
    pub organization: Option<String>,
    pub job_title: Option<String>,
    pub birthday: Option<String>,
    pub notes: Option<String>,
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
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Failed to run the AppleScript: {0}")]
    Script(String),

    #[error("Permission denied to access contacts")]
    PermissionDenied,

    #[error("Timeout waiting for contacts")]
    Timeout,

    #[error("Failed to parse contact data: {0}")]
    ParseError(String),
}

impl Contact {
    /// Get all contacts from macOS Contacts app
    pub async fn get_contacts(app_handle: AppHandle) -> Result<Vec<Contact>, ContactError> {
        // Create a temporary AppleScript file to access contacts
        let script_path = Self::create_contacts_script()?;

        // Run the AppleScript with a timeout
        let output = Self::run_applescript(&script_path).await?;

        // Parse the JSON output to contacts
        let contacts: Vec<Contact> = serde_json::from_str(&output)?;

        // Clean up the temporary script file
        let _ = fs::remove_file(script_path);

        Ok(contacts)
    }

    /// Get a contact image by ID
    pub async fn get_contact_image(contact_id: &str) -> Result<Vec<u8>, ContactError> {
        let script = format!(
            r#"
        osascript -e '
        use framework "Foundation"
        use framework "Contacts"
        use scripting additions

        set contactId to "{}"
        
        set contactStore to current application's CNContactStore's alloc()'s init()
        set keysToFetch to current application's NSArray's arrayWithObject:(current application's CNContactImageDataKey)
        
        try
            set predicate to current application's CNContact's predicateForContactsWithIdentifiers:(current application's NSArray's arrayWithObject:(contactId))
            set contacts to contactStore's unifiedContactsMatchingPredicate:predicate keysToFetch:keysToFetch error:(reference)
            
            if (count of contacts) > 0 then
                set contact to item 1 of contacts
                if contact's imageData() is not missing value then
                    set imageData to contact's imageData()
                    set base64String to (current application's NSString's alloc()'s initWithData:imageData encoding:(current application's NSUTF8StringEncoding))
                    return base64String as text
                end if
            end if
            
            return ""
        on error errMsg
            return ""
        end try
        '
        "#,
            contact_id
        );

        let output = Command::new("sh").arg("-c").arg(&script).output().await?;

        if !output.status.success() {
            return Err(ContactError::Script(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        let base64_data = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if base64_data.is_empty() {
            return Err(ContactError::ParseError("No image available".to_string()));
        }

        // Decode the base64 string to bytes
        let image_data = base64::decode(base64_data)
            .map_err(|e| ContactError::ParseError(format!("Failed to decode image: {}", e)))?;

        Ok(image_data)
    }

    /// Creates a temporary AppleScript file to fetch contacts
    fn create_contacts_script() -> Result<PathBuf, ContactError> {
        let temp_dir = std::env::temp_dir();
        let script_path = temp_dir.join("fetch_contacts.scpt");

        let script = r#"
        use framework "Foundation"
        use framework "Contacts"
        use scripting additions

        -- Create a contact store
        set contactStore to current application's CNContactStore's alloc()'s init()

        -- Define the contact keys we want to fetch
        set keysToFetch to current application's NSArray's arrayWithObjects_count_({current application's CNContactIdentifierKey, current application's CNContactGivenNameKey, current application's CNContactFamilyNameKey, current application's CNContactOrganizationNameKey, current application's CNContactJobTitleKey, current application's CNContactEmailAddressesKey, current application's CNContactPhoneNumbersKey, current application's CNContactPostalAddressesKey, current application's CNContactNoteKey, current application's CNContactBirthdayKey, current application's CNContactImageDataAvailableKey}, 11)

        -- Request permission to access contacts
        set authStatus to contactStore's authorizationStatusForEntityType:(current application's CNEntityTypeContacts)
        
        if authStatus is not equal to (current application's CNAuthorizationStatusAuthorized) then
            contactStore's requestAccessForEntityType:current application's CNEntityTypeContacts completionHandler:(missing value)
            
            set authStatus to contactStore's authorizationStatusForEntityType:(current application's CNEntityTypeContacts)
            if authStatus is not equal to (current application's CNAuthorizationStatusAuthorized) then
                return "{\"error\": \"Permission denied to access contacts\"}"
            end if
        end if

        -- Fetch all contacts
        set fetchRequest to current application's CNContactFetchRequest's alloc()'s initWithKeysToFetch:keysToFetch
        
        set contactsArray to current application's NSMutableArray's alloc()'s init()
        
        try
            contactStore's enumerateContactsWithFetchRequest:fetchRequest error:(reference) usingBlock:(handler |contact| stop |
                set contactDict to current application's NSMutableDictionary's alloc()'s init()
                
                -- Basic info
                contactDict's setValue:contact's identifier() forKey:"id"
                if contact's givenName() is not missing value then
                    contactDict's setValue:contact's givenName() forKey:"given_name"
                end if
                if contact's familyName() is not missing value then
                    contactDict's setValue:contact's familyName() forKey:"family_name"
                end if
                
                -- Full name
                set fullName to ""
                if contact's givenName() is not missing value and contact's familyName() is not missing value then
                    set fullName to contact's givenName() & " " & contact's familyName()
                else if contact's givenName() is not missing value then
                    set fullName to contact's givenName()
                else if contact's familyName() is not missing value then
                    set fullName to contact's familyName()
                end if
                if fullName is not equal to "" then
                    contactDict's setValue:fullName forKey:"name"
                end if
                
                -- Organization and job
                if contact's organizationName() is not missing value then
                    contactDict's setValue:contact's organizationName() forKey:"organization"
                end if
                if contact's jobTitle() is not missing value then
                    contactDict's setValue:contact's jobTitle() forKey:"job_title"
                end if
                
                -- Notes
                if contact's note() is not missing value then
                    contactDict's setValue:contact's note() forKey:"notes"
                end if
                
                -- Image availability
                contactDict's setValue:contact's imageDataAvailable() forKey:"image_available"
                
                -- Birthday
                if contact's birthday() is not missing value then
                    set birthdayComponents to contact's birthday()
                    set year to birthdayComponents's |year|()
                    set month to birthdayComponents's |month|()
                    set day to birthdayComponents's |day|()
                    set birthdayString to year as text & "-" & (month as text) & "-" & (day as text)
                    contactDict's setValue:birthdayString forKey:"birthday"
                end if
                
                -- Email addresses
                set emailsArray to current application's NSMutableArray's alloc()'s init()
                repeat with emailAddress in contact's emailAddresses()
                    set emailDict to current application's NSMutableDictionary's alloc()'s init()
                    
                    set label to emailAddress's label()
                    if label is not missing value then
                        set localizedLabel to current application's CNLabeledValue's localizedStringForLabel:label
                        emailDict's setValue:localizedLabel forKey:"label"
                    else
                        emailDict's setValue:"Email" forKey:"label"
                    end if
                    
                    emailDict's setValue:emailAddress's value() as text forKey:"value"
                    emailsArray's addObject:emailDict
                end repeat
                contactDict's setValue:emailsArray forKey:"emails"
                
                -- Phone numbers
                set phonesArray to current application's NSMutableArray's alloc()'s init()
                repeat with phoneNumber in contact's phoneNumbers()
                    set phoneDict to current application's NSMutableDictionary's alloc()'s init()
                    
                    set label to phoneNumber's label()
                    if label is not missing value then
                        set localizedLabel to current application's CNLabeledValue's localizedStringForLabel:label
                        phoneDict's setValue:localizedLabel forKey:"label"
                    else
                        phoneDict's setValue:"Phone" forKey:"label"
                    end if
                    
                    set formattedNumber to (phoneNumber's value()'s stringValue()) as text
                    phoneDict's setValue:formattedNumber forKey:"value"
                    phonesArray's addObject:phoneDict
                end repeat
                contactDict's setValue:phonesArray forKey:"phone_numbers"
                
                -- Postal addresses
                set addressesArray to current application's NSMutableArray's alloc()'s init()
                repeat with postalAddress in contact's postalAddresses()
                    set addressDict to current application's NSMutableDictionary's alloc()'s init()
                    
                    set label to postalAddress's label()
                    if label is not missing value then
                        set localizedLabel to current application's CNLabeledValue's localizedStringForLabel:label
                        addressDict's setValue:localizedLabel forKey:"label"
                    else
                        addressDict's setValue:"Address" forKey:"label"
                    end if
                    
                    set address to postalAddress's value()
                    
                    if address's street() is not missing value then
                        addressDict's setValue:address's street() as text forKey:"street"
                    end if
                    if address's city() is not missing value then
                        addressDict's setValue:address's city() as text forKey:"city"
                    end if
                    if address's state() is not missing value then
                        addressDict's setValue:address's state() as text forKey:"state"
                    end if
                    if address's postalCode() is not missing value then
                        addressDict's setValue:address's postalCode() as text forKey:"postal_code"
                    end if
                    if address's country() is not missing value then
                        addressDict's setValue:address's country() as text forKey:"country"
                    end if
                    
                    addressesArray's addObject:addressDict
                end repeat
                contactDict's setValue:addressesArray forKey:"addresses"
                
                contactsArray's addObject:contactDict
            end)
            
            set jsonData to current application's NSJSONSerialization's dataWithJSONObject:contactsArray options:0 |error|:(reference)
            set jsonString to (current application's NSString's alloc()'s initWithData:jsonData encoding:(current application's NSUTF8StringEncoding)) as text
            
            return jsonString
        on error errMsg
            return "{\"error\": \"" & errMsg & "\"}"
        end try
        "#;

        fs::write(&script_path, script)?;
        Ok(script_path)
    }

    /// Runs the AppleScript and returns the output
    async fn run_applescript(script_path: &Path) -> Result<String, ContactError> {
        // Run the script with a timeout to prevent hanging
        let script_cmd = format!("osascript '{}'", script_path.display());

        let run_script = async {
            let output = Command::new("sh")
                .arg("-c")
                .arg(&script_cmd)
                .output()
                .await?;

            if !output.status.success() {
                return Err(ContactError::Script(
                    String::from_utf8_lossy(&output.stderr).to_string(),
                ));
            }

            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

            // Check for permission error
            if stdout.contains("Permission denied") {
                return Err(ContactError::PermissionDenied);
            }

            // Check for JSON parsing
            if let Ok(error_obj) = serde_json::from_str::<serde_json::Value>(&stdout) {
                if let Some(error_msg) = error_obj.get("error").and_then(|e| e.as_str()) {
                    return Err(ContactError::Script(error_msg.to_string()));
                }
            }

            Ok(stdout)
        };

        // Add a timeout to prevent hanging if permissions dialog appears
        match timeout(Duration::from_secs(30), run_script).await {
            Ok(result) => result,
            Err(_) => Err(ContactError::Timeout),
        }
    }
}

#[tauri::command]
pub async fn get_contacts(app_handle: AppHandle) -> Result<Vec<Contact>, String> {
    Contact::get_contacts(app_handle)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn fetch_contact_image(contact_id: String) -> Result<Vec<u8>, String> {
    Contact::get_contact_image(&contact_id)
        .await
        .map_err(|e| e.to_string())
}
