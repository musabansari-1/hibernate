//! TCP listener ownership is an observation, never authority to stop a process or proof of a repository.
//! API contract: https://learn.microsoft.com/en-us/windows/win32/api/iphlpapi/nf-iphlpapi-getextendedtcptable
use serde_json::{json, Value};
use std::{
    mem::{offset_of, size_of},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
};
use windows::Win32::{
    Foundation::ERROR_INSUFFICIENT_BUFFER,
    NetworkManagement::IpHelper::*,
    Networking::WinSock::{AF_INET, AF_INET6},
};
type R<T> = Result<T, String>;
struct Table {
    buffer: Vec<u64>,
    bytes: usize,
}
fn table(family: u32) -> R<Table> {
    let mut bytes = 0;
    let mut status = unsafe {
        GetExtendedTcpTable(
            None,
            &mut bytes,
            false,
            family,
            TCP_TABLE_OWNER_PID_LISTENER,
            0,
        )
    };
    for _ in 0..4 {
        if status != ERROR_INSUFFICIENT_BUFFER.0 {
            return Err(format!("Windows listener query failed ({status})"));
        }
        if !(4..=4 * 1024 * 1024).contains(&bytes) {
            return Err("Windows listener table size is outside the supported bound".into());
        }
        // u64 storage provides alignment for all structures used here. Windows can grow the table between calls.
        let mut buffer = vec![0u64; (bytes as usize).div_ceil(8)];
        let capacity = buffer.len() * 8;
        bytes = capacity as u32;
        status = unsafe {
            GetExtendedTcpTable(
                Some(buffer.as_mut_ptr().cast()),
                &mut bytes,
                false,
                family,
                TCP_TABLE_OWNER_PID_LISTENER,
                0,
            )
        };
        if status == 0 {
            if bytes as usize > capacity || bytes < 4 {
                return Err("Windows returned an invalid listener table size".into());
            }
            return Ok(Table {
                buffer,
                bytes: bytes as usize,
            });
        }
    }
    Err("Windows listener table kept changing; ownership is unavailable".into())
}
fn row_offsets(table: &Table, offset: usize, stride: usize) -> R<std::ops::Range<usize>> {
    let count = unsafe { table.buffer.as_ptr().cast::<u32>().read_unaligned() } as usize;
    if count
        .checked_mul(stride)
        .and_then(|n| n.checked_add(offset))
        .is_none_or(|end| end > table.bytes)
    {
        return Err("Windows returned a truncated listener table".into());
    }
    Ok(0..count)
}
fn matches(address: IpAddr, port: u16, expected: SocketAddr) -> bool {
    port == expected.port()
        && address.is_ipv4() == expected.is_ipv4()
        && (address.is_unspecified() || address == expected.ip())
}
fn listeners(expected: SocketAddr) -> R<Vec<Value>> {
    let t = table(if expected.is_ipv4() {
        AF_INET.0 as u32
    } else {
        AF_INET6.0 as u32
    })?;
    let mut found = Vec::new();
    let mut add = |address: IpAddr, port: u32, pid: u32| {
        let port = u16::from_be(port as u16);
        if matches(address, port, expected) {
            let identity = crate::platform::process_identity(pid);
            found.push(
                json!({"pid":pid,"address":address.to_string(),"port":port,"identity":identity}),
            );
        }
    };
    if expected.is_ipv4() {
        let offset = offset_of!(MIB_TCPTABLE_OWNER_PID, table);
        let stride = size_of::<MIB_TCPROW_OWNER_PID>();
        for index in row_offsets(&t, offset, stride)? {
            // Bounds checked above. These C structures contain only integers, with no invalid bit patterns.
            let row = unsafe {
                t.buffer
                    .as_ptr()
                    .cast::<u8>()
                    .add(offset + index * stride)
                    .cast::<MIB_TCPROW_OWNER_PID>()
                    .read_unaligned()
            };
            add(
                IpAddr::V4(Ipv4Addr::from(row.dwLocalAddr.to_ne_bytes())),
                row.dwLocalPort,
                row.dwOwningPid,
            );
        }
    } else {
        let offset = offset_of!(MIB_TCP6TABLE_OWNER_PID, table);
        let stride = size_of::<MIB_TCP6ROW_OWNER_PID>();
        for index in row_offsets(&t, offset, stride)? {
            let row = unsafe {
                t.buffer
                    .as_ptr()
                    .cast::<u8>()
                    .add(offset + index * stride)
                    .cast::<MIB_TCP6ROW_OWNER_PID>()
                    .read_unaligned()
            };
            add(
                IpAddr::V6(Ipv6Addr::from(row.ucLocalAddr)),
                row.dwLocalPort,
                row.dwOwningPid,
            );
        }
    }
    Ok(found)
}
pub fn observe(endpoint: SocketAddr) -> Value {
    match listeners(endpoint) {
        Ok(items) => {
            json!({"status":if items.is_empty(){"unobserved"}else{"observed"},"endpoint":endpoint.to_string(),"checkedAt":crate::now(),"listeners":items,"projectOwnership":"unverified","detail":"Windows listener ownership sampled for the connected endpoint. A listening process does not establish repository ownership or prove which process served this response. No process may be stopped on this evidence alone."})
        }
        Err(e) => {
            json!({"status":"unavailable","endpoint":endpoint.to_string(),"checkedAt":crate::now(),"listeners":[],"projectOwnership":"unverified","detail":e})
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn observes_real_ipv4_and_ipv6_listener_pid_and_creation_identity() {
        for address in ["127.0.0.1:0", "[::1]:0"] {
            let socket = std::net::TcpListener::bind(address).unwrap();
            let endpoint = socket.local_addr().unwrap();
            let observation = observe(endpoint);
            assert_eq!(observation["status"], "observed", "{observation}");
            assert!(
                observation["listeners"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|v| v["pid"] == std::process::id()
                        && v["identity"]["processStarted"].is_string()
                        && v["identity"]["executable"].is_string()),
                "{observation}"
            );
            assert_eq!(observation["projectOwnership"], "unverified");
        }
    }
    #[test]
    fn rejects_truncated_rows_and_unrelated_interfaces() {
        let t = Table {
            buffer: vec![100],
            bytes: 8,
        };
        assert!(row_offsets(&t, 4, size_of::<MIB_TCPROW_OWNER_PID>()).is_err());
        let endpoint = "127.0.0.1:3000".parse().unwrap();
        assert!(matches("0.0.0.0".parse().unwrap(), 3000, endpoint));
        assert!(!matches("127.0.0.2".parse().unwrap(), 3000, endpoint));
        assert!(!matches("::".parse().unwrap(), 3000, endpoint));
        assert!(!matches("127.0.0.1".parse().unwrap(), 8000, endpoint));
    }
}
