//! DPAPI protects semantic payloads for the current Windows account. No machine-wide key.
use serde_json::Value;
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::{LocalFree, HLOCAL},
        Security::Cryptography::{
            CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
        },
    },
};
type R<T> = Result<T, String>;
const PREFIX: &str = "dpapi:v1:";
fn transform(bytes: &[u8], encrypt: bool) -> R<Vec<u8>> {
    if bytes.len() > 8_000_000 {
        return Err("Protected payload exceeds the local storage limit".into());
    }
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        if encrypt{CryptProtectData(&input,PCWSTR::null(),None,None,None,CRYPTPROTECT_UI_FORBIDDEN,&mut output)}else{CryptUnprotectData(&input,None,None,None,None,CRYPTPROTECT_UI_FORBIDDEN,&mut output)}.map_err(|e|format!("Windows data protection failed. Use the original Windows account; no data was replaced. {e}"))?;
        struct Allocation(*mut u8);
        impl Drop for Allocation {
            fn drop(&mut self) {
                unsafe {
                    let _ = LocalFree(Some(HLOCAL(self.0 as *mut _)));
                }
            }
        }
        let _allocation = Allocation(output.pbData);
        if output.pbData.is_null() {
            return Err("Windows returned an empty protected payload".into());
        }
        Ok(std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec())
    }
}
pub fn encode(value: &Value) -> R<String> {
    let encrypted = transform(value.to_string().as_bytes(), true)?;
    let mut result = String::with_capacity(PREFIX.len() + encrypted.len() * 2);
    result.push_str(PREFIX);
    use std::fmt::Write;
    for byte in encrypted {
        write!(result, "{byte:02x}").map_err(|e| e.to_string())?;
    }
    Ok(result)
}
pub fn decode(raw: &str) -> R<Value> {
    let Some(hex) = raw.strip_prefix(PREFIX) else {
        return serde_json::from_str(raw).map_err(|e| format!("Stored JSON is corrupt: {e}"));
    };
    if hex.len() > 16_000_000 || !hex.len().is_multiple_of(2) || !hex.is_ascii() {
        return Err("Protected payload is corrupt".into());
    }
    let bytes = hex
        .as_bytes()
        .chunks_exact(2)
        .map(|s| {
            u8::from_str_radix(std::str::from_utf8(s).unwrap_or(""), 16)
                .map_err(|_| "Protected payload is corrupt".to_string())
        })
        .collect::<R<Vec<_>>>()?;
    let plain = transform(&bytes, false)?;
    serde_json::from_slice(&plain).map_err(|_| "Decrypted payload is not a valid snapshot".into())
}
pub fn sql_decode(raw: String) -> rusqlite::Result<Value> {
    decode(&raw).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            std::io::Error::new(std::io::ErrorKind::InvalidData, e).into(),
        )
    })
}
pub fn migrate(c: &rusqlite::Connection) -> R<()> {
    use rusqlite::OptionalExtension;
    if c.query_row(
        "SELECT body FROM meta WHERE key='payloadProtection'",
        [],
        |r| r.get::<_, String>(0),
    )
    .optional()
    .map_err(|e| e.to_string())?
    .as_deref()
        == Some("dpapi-v1")
    {
        return Ok(());
    }
    // Identifiers below are fixed, never supplied by an application integration.
    for (table, column) in [
        ("snapshots", "body"),
        ("snapshots", "result"),
        ("reports", "body"),
        ("projects", "body"),
        ("services", "body"),
        ("job_context", "project"),
        ("jobs", "result"),
    ] {
        let mut q = c
            .prepare(&format!(
                "SELECT rowid,{column} FROM {table} WHERE {column} IS NOT NULL"
            ))
            .map_err(|e| e.to_string())?;
        let rows = q
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        for (id, value) in rows {
            if !value.starts_with(PREFIX) {
                c.execute(
                    &format!("UPDATE {table} SET {column}=? WHERE rowid=?"),
                    rusqlite::params![encode(&decode(&value)?)?, id],
                )
                .map_err(|e| e.to_string())?;
            }
        }
    }
    c.execute("DELETE FROM meta WHERE key='workspace'", [])
        .map_err(|e| e.to_string())?;
    c.execute(
        "INSERT OR REPLACE INTO meta VALUES('payloadProtection','dpapi-v1')",
        [],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn account_bound_roundtrip_and_corruption_refusal() {
        let value = serde_json::json!({"url":"http://localhost:3000/private"});
        let encrypted = encode(&value).unwrap();
        assert!(!encrypted.contains("localhost"));
        assert_eq!(decode(&encrypted).unwrap(), value);
        assert!(decode("dpapi:v1:000000").is_err());
        assert!(decode("damaged-json").is_err());
    }
}
