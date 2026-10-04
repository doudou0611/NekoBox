//! Account tokens stay in the native vault; never serialize them to IPC.
use super::*;
use sha2::{Digest, Sha256};
pub trait Vault {
    fn read(&self) -> Result<Option<String>>;
    fn write(&self, value: &str) -> Result<()>;
    fn delete(&self) -> Result<()>;
}
pub struct NativeVault(keyring::Entry);
fn error() -> ServiceError {
    ServiceError(ErrorCode::PermissionDenied, "无法访问账户的系统凭据库。")
}
impl NativeVault {
    pub fn new(backend: &Backend, service: &str) -> Result<Self> {
        let scope = format!(
            "{:x}",
            Sha256::digest(path_text(&backend.data_directory)?.as_bytes())
        );
        keyring::Entry::new(service, &scope)
            .map(Self)
            .map_err(|_| error())
    }
}
impl Vault for NativeVault {
    fn read(&self) -> Result<Option<String>> {
        match self.0.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(error()),
        }
    }
    fn write(&self, value: &str) -> Result<()> {
        self.0.set_password(value).map_err(|_| error())
    }
    fn delete(&self) -> Result<()> {
        match self.0.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(error()),
        }
    }
}
pub fn validate(value: &str) -> Result<&str> {
    let value = value.trim();
    if value.is_empty() || value.len() > 8192 || !value.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(invalid("授权内容无效，请检查输入。"));
    }
    Ok(value)
}
#[cfg(test)]
#[derive(Default)]
pub struct MemoryVault(pub std::cell::RefCell<Option<String>>);
#[cfg(test)]
impl Vault for MemoryVault {
    fn read(&self) -> Result<Option<String>> {
        Ok(self.0.borrow().clone())
    }
    fn write(&self, value: &str) -> Result<()> {
        *self.0.borrow_mut() = Some(value.into());
        Ok(())
    }
    fn delete(&self) -> Result<()> {
        *self.0.borrow_mut() = None;
        Ok(())
    }
}
