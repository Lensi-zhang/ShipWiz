use crate::error::AppError;
use keyring::Entry;
use serde::{Deserialize, Serialize};

const SERVICE_NAME: &str = "shipwiz";

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredCredential {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "privateKey"
    )]
    pub private_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub passphrase: Option<String>,
}

impl StoredCredential {
    pub fn is_empty(&self) -> bool {
        self.password.is_none() && self.private_key.is_none() && self.passphrase.is_none()
    }
}

fn entry(credential_id: &str) -> Result<Entry, AppError> {
    Entry::new(SERVICE_NAME, credential_id).map_err(|error| {
        AppError::credential(format!("无法访问系统密钥库，请检查系统凭据服务：{error}"))
    })
}

pub fn load(credential_id: &str) -> Result<StoredCredential, AppError> {
    match entry(credential_id)?.get_password() {
        Ok(value) => serde_json::from_str(&value).map_err(|error| {
            AppError::credential(format!(
                "系统密钥库中的凭据格式无效，请重新编辑节点凭据：{error}"
            ))
        }),
        Err(keyring::Error::NoEntry) => Ok(StoredCredential::default()),
        Err(error) => Err(AppError::credential(format!(
            "读取系统密钥库失败，请检查系统凭据服务：{error}"
        ))),
    }
}

pub fn save(credential_id: &str, credential: &StoredCredential) -> Result<(), AppError> {
    let value = serde_json::to_string(credential)
        .map_err(|error| AppError::credential(format!("整理节点凭据失败：{error}")))?;
    entry(credential_id)?.set_password(&value).map_err(|error| {
        AppError::credential(format!(
            "保存到系统密钥库失败，节点秘密未写入配置文件：{error}"
        ))
    })
}

pub fn delete(credential_id: &str) -> Result<(), AppError> {
    match entry(credential_id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(AppError::credential(format!(
            "删除系统密钥库中的节点凭据失败：{error}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_json_only_contains_supplied_secrets() {
        let value = serde_json::to_value(StoredCredential {
            password: Some("secret".to_string()),
            ..StoredCredential::default()
        })
        .unwrap();
        assert_eq!(value, serde_json::json!({ "password": "secret" }));
        assert!(StoredCredential::default().is_empty());
    }
}
