//! Explicit loopback observations. HTTP redirects are never followed and responses are bounded.
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    net::{IpAddr, SocketAddr, TcpStream},
    time::{Duration, Instant},
};
type R<T> = Result<T, String>;
pub(crate) fn endpoint(raw: &str) -> R<(url::Url, Vec<SocketAddr>)> {
    let u =
        url::Url::parse(raw).map_err(|_| "Enter an http://localhost:port/path health endpoint")?;
    if u.scheme() != "http"
        || !u.username().is_empty()
        || u.password().is_some()
        || u.fragment().is_some()
        || u.query().is_some()
    {
        return Err("Health checks require plain HTTP loopback URLs without credentials, query strings or fragments".into());
    }
    let hosts = match u.host_str().unwrap_or("") {
        "localhost" => vec![
            IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
            IpAddr::V6(std::net::Ipv6Addr::LOCALHOST),
        ],
        "127.0.0.1" => vec![IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)],
        "[::1]" => vec![IpAddr::V6(std::net::Ipv6Addr::LOCALHOST)],
        _ => {
            return Err(
                "Only localhost, 127.0.0.1 and [::1] are allowed; no DNS lookup is performed"
                    .into(),
            )
        }
    };
    let port = u.port_or_known_default().ok_or("Missing port")?;
    if port == 0 {
        return Err("Port must be between 1 and 65535".into());
    }
    Ok((
        u,
        hosts
            .into_iter()
            .map(|ip| SocketAddr::new(ip, port))
            .collect(),
    ))
}
pub fn check(raw: &str) -> R<Value> {
    let (u, addresses) = endpoint(raw)?;
    let start = Instant::now();
    let stream = addresses
        .iter()
        .find_map(|a| TcpStream::connect_timeout(a, Duration::from_millis(700)).ok());
    let Some(mut stream) = stream else {
        return Ok(
            json!({"url":raw,"status":"missing","listening":false,"httpStatus":null,"checkedAt":crate::now(),"detail":"No connection to the expected loopback endpoint. Service startup is not verified."}),
        );
    };
    let listener_ownership = stream.peer_addr().ok().map(crate::listeners::observe);
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;
    let request=format!("GET {} HTTP/1.1\r\nHost: {}:{}\r\nConnection: close\r\nUser-Agent: ProjectHibernate-Health/0.1\r\n\r\n",u.path(),u.host_str().unwrap_or("localhost"),u.port_or_known_default().unwrap_or(80));
    let mut response = Vec::new();
    let response_result = (|| -> std::io::Result<()> {
        stream.write_all(request.as_bytes())?;
        let deadline = Instant::now() + Duration::from_secs(3);
        while response.len() < 4096 && Instant::now() < deadline {
            let mut b = [0u8; 256];
            let n = stream.read(&mut b)?;
            if n == 0 {
                break;
            }
            response.extend_from_slice(&b[..n]);
            if response.contains(&b'\n') {
                break;
            }
        }
        Ok(())
    })();
    let text = String::from_utf8_lossy(&response);
    let mut parts = text.lines().next().unwrap_or("").split_whitespace();
    let protocol = parts.next().unwrap_or("");
    let code = parts
        .next()
        .filter(|s| s.len() == 3 && s.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|s| s.parse::<u16>().ok())
        .filter(|n| {
            (100..600).contains(n)
                && (protocol == "HTTP/1.0" || protocol == "HTTP/1.1")
                && response.windows(2).any(|w| w == b"\r\n")
                && response_result.is_ok()
        });
    let healthy = code.is_some_and(|n| (200..300).contains(&n));
    Ok(
        json!({"url":raw,"status":if healthy{"healthy"}else{"unhealthy"},"listening":true,"listenerOwnership":listener_ownership,"httpStatus":code,"elapsedMs":start.elapsed().as_millis(),"checkedAt":crate::now(),"detail":if healthy{"Expected endpoint returned HTTP 2xx. This verifies endpoint response, not process ownership or application correctness."}else if response_result.is_err(){"Port accepts connections but did not return a complete HTTP status within the timeout."}else{"Expected endpoint did not return HTTP 2xx. Redirects are not followed."}}),
    )
}
pub fn save(project_id: &str, name: &str, url: &str) -> R<Value> {
    endpoint(url)?;
    if name.trim().is_empty() || name.len() > 120 {
        return Err("Enter a short service name".into());
    }
    let c = crate::store::open()?;
    crate::projects::get(&c, project_id)?;
    let id = uuid::Uuid::new_v4().to_string();
    let value = json!({"id":id,"projectId":project_id,"name":name.trim(),"url":url});
    c.execute(
        "INSERT INTO services VALUES(?,?,?)",
        rusqlite::params![id, project_id, crate::protection::encode(&value)?],
    )
    .map_err(|e| e.to_string())?;
    Ok(value)
}
pub fn list(c: &rusqlite::Connection, project_id: &str) -> R<Vec<Value>> {
    let mut q = c
        .prepare("SELECT body FROM services WHERE project_id=? ORDER BY rowid")
        .map_err(|e| e.to_string())?;
    let result = q
        .query_map([project_id], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .map(|s| crate::protection::decode(&s.map_err(|e| e.to_string())?))
        .collect();
    result
}
pub fn verify_project(project_id: &str) -> R<Value> {
    let c = crate::store::open()?;
    let services = list(&c, project_id)?;
    let results = verify_definitions(&services);
    let value = json!({"projectId":project_id,"checkedAt":crate::now(),"services":results});
    c.execute(
        "INSERT OR REPLACE INTO meta VALUES(?,?)",
        rusqlite::params![
            format!("health:{project_id}"),
            crate::protection::encode(&value)?
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(value)
}
pub fn verify_definitions(services: &[Value]) -> Vec<Value> {
    services
        .iter()
        .map(|s| {
            let observed = check(s["url"].as_str().unwrap_or(""))
                .unwrap_or_else(|e| json!({"status":"error","detail":e}));
            json!({"id":s["id"],"name":s["name"],"observation":observed})
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restricts_network_authority() {
        for u in [
            "https://localhost:3000",
            "http://example.com",
            "http://127.0.0.2",
            "http://localhost.evil.test",
            "http://user:pass@localhost",
            "http://localhost/?token=x",
            "http://localhost:0",
        ] {
            assert!(endpoint(u).is_err(), "{u}");
        }
    }
    #[test]
    fn observes_http_status_without_following_redirects() {
        for (status, expected) in [
            ("200 OK", "healthy"),
            ("302 Found", "unhealthy"),
            ("503 Unavailable", "unhealthy"),
        ] {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let thread = std::thread::spawn(move || {
                let (mut s, _) = listener.accept().unwrap();
                let mut request = [0; 1024];
                assert!(s.read(&mut request).unwrap() > 0);
                s.write_all(format!("HTTP/1.1 {status}\r\nLocation: http://example.com/\r\nContent-Length: 0\r\n\r\n").as_bytes()).unwrap();
            });
            let result = check(&format!("http://{address}/health")).unwrap();
            assert_eq!(result["status"], expected);
            assert_eq!(result["listening"], true);
            assert_eq!(result["listenerOwnership"]["status"], "observed");
            assert!(result["listenerOwnership"]["listeners"]
                .as_array()
                .unwrap()
                .iter()
                .any(|p| p["pid"] == std::process::id()));
            assert_eq!(
                result["listenerOwnership"]["projectOwnership"],
                "unverified"
            );
            thread.join().unwrap();
        }
    }
    #[test]
    fn closed_port_is_missing() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        assert_eq!(
            check(&format!("http://{addr}/")).unwrap()["status"],
            "missing"
        );
    }
    #[test]
    fn partial_or_malformed_status_is_not_healthy() {
        for line in [
            "HTTP/1.1 200",
            "HTTP/1.1 +200 OK\r\n",
            "HTTP/1.1 0200 OK\r\n",
        ] {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            let thread = std::thread::spawn(move || {
                let (mut s, _) = listener.accept().unwrap();
                let mut b = [0u8; 1024];
                let _ = s.read(&mut b);
                s.write_all(line.as_bytes()).unwrap();
            });
            let result = check(&format!("http://{addr}/health")).unwrap();
            assert_eq!(result["status"], "unhealthy");
            assert_eq!(result["httpStatus"], Value::Null);
            thread.join().unwrap();
        }
    }
}
