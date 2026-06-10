use rusqlite::Connection;

pub const SMTP_TARGET: &str = "si.upn-generator.smtp.password";
pub const IMAP_TARGET: &str = "si.upn-generator.imap.password";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailCredentialKind {
    Smtp,
    Imap,
}

impl MailCredentialKind {
    pub fn target(self) -> &'static str {
        match self {
            Self::Smtp => SMTP_TARGET,
            Self::Imap => IMAP_TARGET,
        }
    }

    fn table(self) -> &'static str {
        match self {
            Self::Smtp => "smtp_config",
            Self::Imap => "inbox_config",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Smtp => "SMTP",
            Self::Imap => "IMAP",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoredCredential {
    Found { username: String, password: String },
    NotFound,
    ReadError(String),
}

trait CredentialBackend {
    fn read(&self, target: &str) -> StoredCredential;
    fn write(&self, target: &str, username: &str, password: &str) -> Result<(), String>;
    fn delete(&self, target: &str) -> Result<(), String>;
}

pub struct WindowsCredentialBackend;

#[cfg(windows)]
impl CredentialBackend for WindowsCredentialBackend {
    fn read(&self, target: &str) -> StoredCredential {
        windows_credential::read(target)
    }

    fn write(&self, target: &str, username: &str, password: &str) -> Result<(), String> {
        windows_credential::write(target, username, password)
    }

    fn delete(&self, target: &str) -> Result<(), String> {
        windows_credential::delete(target)
    }
}

#[cfg(not(windows))]
impl CredentialBackend for WindowsCredentialBackend {
    fn read(&self, _target: &str) -> StoredCredential {
        StoredCredential::NotFound
    }

    fn write(&self, _target: &str, _username: &str, _password: &str) -> Result<(), String> {
        Err("Windows Credential Manager is not available on this platform.".to_string())
    }

    fn delete(&self, _target: &str) -> Result<(), String> {
        Ok(())
    }
}

fn clear_db_password(conn: &Connection, kind: MailCredentialKind) -> Result<(), String> {
    conn.execute(
        &format!("UPDATE {} SET password='' WHERE id=1", kind.table()),
        [],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn resolve_password_with_backend<B: CredentialBackend>(
    backend: &B,
    kind: MailCredentialKind,
    username: &str,
    explicit_password: &str,
) -> Result<String, String> {
    if !explicit_password.is_empty() {
        return Ok(explicit_password.to_string());
    }

    match backend.read(kind.target()) {
        StoredCredential::Found {
            username: stored_username,
            password,
        } => {
            if stored_username == username {
                return Ok(password);
            }
        }
        StoredCredential::NotFound => {}
        StoredCredential::ReadError(error) => {
            eprintln!("{} Credential Manager read failed: {}", kind.label(), error);
        }
    }

    Err(format!("{} password not configured.", kind.label()))
}

pub fn resolve_password(
    kind: MailCredentialKind,
    username: &str,
    explicit_password: &str,
) -> Result<String, String> {
    resolve_password_with_backend(&WindowsCredentialBackend, kind, username, explicit_password)
}

fn password_configured_with_backend<B: CredentialBackend>(
    backend: &B,
    kind: MailCredentialKind,
    username: &str,
) -> Result<bool, String> {
    match backend.read(kind.target()) {
        StoredCredential::Found {
            username: stored_username,
            ..
        } => {
            if stored_username == username {
                return Ok(true);
            }
        }
        StoredCredential::NotFound => {}
        StoredCredential::ReadError(error) => {
            eprintln!(
                "{} Credential Manager read failed while checking saved state: {}",
                kind.label(),
                error
            );
        }
    }

    Ok(false)
}

pub fn password_configured(kind: MailCredentialKind, username: &str) -> Result<bool, String> {
    password_configured_with_backend(&WindowsCredentialBackend, kind, username)
}

pub fn save_password(
    conn: &Connection,
    kind: MailCredentialKind,
    username: &str,
    password: &str,
) -> Result<(), String> {
    if password.is_empty() {
        return Ok(());
    }
    WindowsCredentialBackend.write(kind.target(), username, password)?;
    clear_db_password(conn, kind)?;
    Ok(())
}

pub fn delete_mail_credentials() -> Result<(), String> {
    let backend = WindowsCredentialBackend;
    let mut errors = Vec::new();
    for kind in [MailCredentialKind::Smtp, MailCredentialKind::Imap] {
        if let Err(error) = backend.delete(kind.target()) {
            errors.push(format!("{}: {}", kind.label(), error));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Could not delete saved mail credentials: {}",
            errors.join("; ")
        ))
    }
}

#[cfg(windows)]
mod windows_credential {
    use super::StoredCredential;
    use std::ptr::null_mut;
    use windows::core::{Error, PCWSTR, PWSTR};
    use windows::Win32::Security::Credentials::{
        CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE,
        CRED_TYPE_GENERIC,
    };

    const HRESULT_FROM_WIN32_ERROR_NOT_FOUND: u32 = 0x8007_0490;

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    unsafe fn pwstr_to_string(ptr: PWSTR) -> String {
        if ptr.is_null() {
            return String::new();
        }
        let mut len = 0usize;
        while *ptr.0.add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(ptr.0, len))
    }

    pub fn read(target: &str) -> StoredCredential {
        let target_w = wide(target);
        let mut credential: *mut CREDENTIALW = null_mut();
        let result = unsafe {
            CredReadW(
                PCWSTR(target_w.as_ptr()),
                CRED_TYPE_GENERIC,
                None,
                &mut credential,
            )
        };

        if let Err(error) = result {
            if error.code().0 as u32 == HRESULT_FROM_WIN32_ERROR_NOT_FOUND {
                return StoredCredential::NotFound;
            }
            return StoredCredential::ReadError(error.to_string());
        }

        if credential.is_null() {
            return StoredCredential::ReadError(
                "Credential Manager returned a null credential.".to_string(),
            );
        }

        let stored = unsafe {
            let credential_ref = &*credential;
            let username = pwstr_to_string(credential_ref.UserName);
            let bytes = std::slice::from_raw_parts(
                credential_ref.CredentialBlob,
                credential_ref.CredentialBlobSize as usize,
            );
            let password = match String::from_utf8(bytes.to_vec()) {
                Ok(password) => password,
                Err(error) => {
                    CredFree(credential.cast());
                    return StoredCredential::ReadError(format!(
                        "Credential blob is not valid UTF-8: {}",
                        error
                    ));
                }
            };
            CredFree(credential.cast());
            StoredCredential::Found { username, password }
        };
        stored
    }

    pub fn write(target: &str, username: &str, password: &str) -> Result<(), String> {
        let mut target_w = wide(target);
        let mut username_w = wide(username);
        let mut password_bytes = password.as_bytes().to_vec();
        let mut credential = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(target_w.as_mut_ptr()),
            CredentialBlobSize: password_bytes
                .len()
                .try_into()
                .map_err(|_| "Credential is too large.".to_string())?,
            CredentialBlob: password_bytes.as_mut_ptr(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            UserName: PWSTR(username_w.as_mut_ptr()),
            ..Default::default()
        };

        unsafe { CredWriteW(&mut credential, 0) }.map_err(|e| e.to_string())
    }

    pub fn delete(target: &str) -> Result<(), String> {
        let target_w = wide(target);
        match unsafe { CredDeleteW(PCWSTR(target_w.as_ptr()), CRED_TYPE_GENERIC, None) } {
            Ok(()) => Ok(()),
            Err(error) if error.code().0 as u32 == HRESULT_FROM_WIN32_ERROR_NOT_FOUND => Ok(()),
            Err(error) => Err(Error::from(error.code()).to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    struct FakeBackend {
        entries: RefCell<HashMap<String, StoredCredential>>,
    }

    impl FakeBackend {
        fn new() -> Self {
            Self {
                entries: RefCell::new(HashMap::new()),
            }
        }
    }

    impl CredentialBackend for FakeBackend {
        fn read(&self, target: &str) -> StoredCredential {
            self.entries
                .borrow()
                .get(target)
                .cloned()
                .unwrap_or(StoredCredential::NotFound)
        }

        fn write(&self, target: &str, username: &str, password: &str) -> Result<(), String> {
            self.entries.borrow_mut().insert(
                target.to_string(),
                StoredCredential::Found {
                    username: username.to_string(),
                    password: password.to_string(),
                },
            );
            Ok(())
        }

        fn delete(&self, target: &str) -> Result<(), String> {
            self.entries.borrow_mut().remove(target);
            Ok(())
        }
    }

    #[test]
    fn resolver_prefers_explicit_password() {
        let backend = FakeBackend::new();

        let password = resolve_password_with_backend(
            &backend,
            MailCredentialKind::Smtp,
            "user@example.com",
            "typed",
        )
        .unwrap();

        assert_eq!(password, "typed");
    }

    #[test]
    fn resolver_uses_matching_credential_manager_secret() {
        let backend = FakeBackend::new();
        backend
            .write(SMTP_TARGET, "user@example.com", "saved")
            .unwrap();

        let password = resolve_password_with_backend(
            &backend,
            MailCredentialKind::Smtp,
            "user@example.com",
            "",
        )
        .unwrap();

        assert_eq!(password, "saved");
    }

    #[test]
    fn resolver_ignores_mismatched_credential_username() {
        let backend = FakeBackend::new();
        backend
            .write(SMTP_TARGET, "old@example.com", "saved")
            .unwrap();

        let error = resolve_password_with_backend(
            &backend,
            MailCredentialKind::Smtp,
            "user@example.com",
            "",
        )
        .unwrap_err();

        assert_eq!(error, "SMTP password not configured.");
    }

    #[test]
    fn resolver_errors_on_read_error_without_legacy_fallback() {
        let backend = FakeBackend::new();
        backend.entries.borrow_mut().insert(
            SMTP_TARGET.to_string(),
            StoredCredential::ReadError("denied".to_string()),
        );

        let error = resolve_password_with_backend(
            &backend,
            MailCredentialKind::Smtp,
            "user@example.com",
            "",
        )
        .unwrap_err();

        assert_eq!(error, "SMTP password not configured.");
    }

    #[test]
    fn resolver_errors_when_no_password_is_available() {
        let backend = FakeBackend::new();

        let error = resolve_password_with_backend(
            &backend,
            MailCredentialKind::Smtp,
            "user@example.com",
            "",
        )
        .unwrap_err();

        assert_eq!(error, "SMTP password not configured.");
    }
}
