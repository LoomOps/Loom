//! Compatible private HTTP-over-Unix framing with bounded bodies and peer credentials.
use crate::executor::{Docker, Executor};
use loom_core::{error::require, Error, Result};
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    os::unix::{
        fs::{FileTypeExt, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};
pub fn call(socket: &Path, credential: &Path, action: &str, body: &Value) -> Result<Value> {
    require(
        action.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'),
        "action",
        "Invalid host action",
        400,
    )?;
    let credential = loom_core::file_set::bounded(credential, 4096)?;
    let token = std::str::from_utf8(&credential)
        .map_err(|_| Error::input("Invalid executor credential encoding"))?
        .trim();
    require(
        !token.is_empty() && !token.contains(['\r', '\n']),
        "credential",
        "Invalid executor credential",
        403,
    )?;
    let mut stream = UnixStream::connect(socket)?;
    stream.set_read_timeout(Some(Duration::from_secs(50)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    let raw = serde_json::to_vec(body)?;
    require(
        raw.len() <= 131072,
        "request",
        "Executor request exceeds bound",
        400,
    )?;
    write!(stream,"POST /{action} HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",raw.len())?;
    stream.write_all(&raw)?;
    let mut reply = vec![];
    stream
        .take(4 * 1024 * 1024 + 8193)
        .read_to_end(&mut reply)?;
    require(
        reply.len() <= 4 * 1024 * 1024 + 8192,
        "reply",
        "Executor response exceeds bound",
        503,
    )?;
    let boundary = reply
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| Error::new("protocol", "Malformed executor response", 503))?;
    let headers = String::from_utf8_lossy(&reply[..boundary]);
    let status = headers
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(503);
    let value: Value = serde_json::from_slice(&reply[boundary + 4..])?;
    require(
        status == 200,
        "executor",
        "Executor rejected or could not confirm operation",
        status,
    )?;
    Ok(value)
}
fn peer(stream: &UnixStream) -> Result<u32> {
    use std::os::fd::AsRawFd;
    let mut creds = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    let rc = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut creds as *mut libc::ucred).cast(),
            &mut len,
        )
    };
    require(rc == 0, "peer", "Cannot verify executor peer", 403)?;
    Ok(creds.uid)
}
fn request(stream: &mut UnixStream) -> Result<(String, String, Value)> {
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    let mut header = Vec::new();
    let mut b = [0; 1];
    while !header.ends_with(b"\r\n\r\n") {
        require(
            header.len() < 16384,
            "headers",
            "Executor headers exceed bound",
            400,
        )?;
        stream.read_exact(&mut b)?;
        header.push(b[0]);
    }
    let header = String::from_utf8(header).map_err(|_| Error::input("Invalid HTTP header"))?;
    let mut lines = header.lines();
    let first = lines
        .next()
        .ok_or_else(|| Error::input("Missing request line"))?;
    let parts: Vec<_> = first.split_whitespace().collect();
    require(
        parts.len() == 3 && parts[0] == "POST" && parts[1].starts_with('/'),
        "method",
        "Executor requires POST",
        405,
    )?;
    let action = parts[1].trim_start_matches('/').to_string();
    let mut token = None;
    let mut length = None;
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            match name.to_ascii_lowercase().as_str() {
                "authorization" => {
                    require(
                        token.is_none(),
                        "credential",
                        "Duplicate authorization",
                        403,
                    )?;
                    token = value.trim().strip_prefix("Bearer ").map(String::from);
                }
                "content-length" => {
                    require(length.is_none(), "length", "Duplicate content length", 400)?;
                    length = value.trim().parse::<usize>().ok();
                }
                "transfer-encoding" => {
                    return Err(Error::input("Chunked executor requests unsupported"))
                }
                _ => {}
            }
        }
    }
    let length = length.ok_or_else(|| Error::input("Content length required"))?;
    require(
        length > 0 && length <= 131072,
        "request",
        "Executor request exceeds bound",
        400,
    )?;
    let mut body = vec![0; length];
    stream.read_exact(&mut body)?;
    Ok((
        action,
        token.ok_or_else(|| Error::new("credential", "Executor token required", 403))?,
        serde_json::from_slice(&body)?,
    ))
}
pub fn serve<D: Docker + 'static>(executor: Executor<D>) -> Result<()> {
    use std::os::fd::AsRawFd;
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(executor.root.join("executor.lock"))?;
    require(
        unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0,
        "busy",
        "Another host Executor owns this root",
        409,
    )?;
    let folder = executor.root.join("socket");
    std::fs::create_dir_all(&folder)?;
    let socket = folder.join("executor.sock");
    if let Ok(meta) = socket.symlink_metadata() {
        require(
            meta.file_type().is_socket(),
            "socket",
            "Refusing non-socket destination",
            409,
        )?;
        require(
            UnixStream::connect(&socket).is_err(),
            "socket",
            "Executor socket already active",
            409,
        )?;
        std::fs::remove_file(&socket)?;
    }
    let listener = UnixListener::bind(&socket)?;
    std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o660))?;
    let c = std::ffi::CString::new(socket.as_os_str().as_encoded_bytes())
        .map_err(|_| Error::input("Invalid socket path"))?;
    require(
        unsafe { libc::chown(c.as_ptr(), executor.config.uid, executor.config.gid) } == 0,
        "socket",
        "Cannot set socket ownership",
        503,
    )?;
    for uid in executor
        .config
        .instances
        .values()
        .filter(|i| i.gid != executor.config.gid)
        .map(|i| i.uid)
        .collect::<std::collections::BTreeSet<_>>()
    {
        let status = std::process::Command::new("setfacl")
            .args(["-m", &format!("u:{uid}:rw")])
            .arg(&socket)
            .status()?;
        require(
            status.success(),
            "socket_acl",
            "Cannot grant registered instance socket access",
            503,
        )?;
    }
    let shared = Arc::new(Mutex::new(executor));
    let (send, receive) = std::sync::mpsc::sync_channel::<UnixStream>(32);
    let receive = Arc::new(Mutex::new(receive));
    let log_readers = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut threads = vec![];
    for _ in 0..4 {
        let receive = receive.clone();
        let shared = shared.clone();
        let log_readers = log_readers.clone();
        threads.push(std::thread::spawn(move||loop{let Ok(mut stream)=receive.lock().unwrap().recv() else{break};let result=(||->Result<Value>{let uid=peer(&stream)?;let (action,token,body)=request(&mut stream)?;let mut executor=shared.lock().map_err(|_|Error::new("executor","Executor unavailable",503))?;let instance=executor.authenticate(&token,uid)?;
if matches!(action.as_str(),"runtime_file_page"|"runtime_file_read") {
    use std::sync::atomic::Ordering;
    require(log_readers.try_update(Ordering::AcqRel,Ordering::Acquire,|n|if n<2{Some(n+1)}else{None}).is_ok(),"busy","File/log readers are busy",503)?;
    let capability=crate::runtime_files::grant(&executor.root,&executor.config,&instance,&body["runtime_ref"]);drop(executor);
    let result=capability.and_then(|c|c.call(&action,&body));log_readers.fetch_sub(1,Ordering::AcqRel);result
} else if matches!(action.as_str(),"logs"|"log_range"|"log_expire") {
    use std::sync::atomic::Ordering;
    require(log_readers.try_update(Ordering::AcqRel,Ordering::Acquire,|n|if n<2{Some(n+1)}else{None}).is_ok(),"busy","Log readers are busy",503)?;
    let capability=executor.log_read(&instance,&body);drop(executor);
    let mut args=body;if action=="logs"{args["tail"]=json!(true);args["limit"]=json!(1024*1024);}
    let result=capability.and_then(|c|if action=="log_expire"{c.expire(&args)}else{c.read(&args)});log_readers.fetch_sub(1,Ordering::AcqRel);result
} else {executor.handle(&instance,&action,body)}})();let (status,body)=match result{Ok(v)=>(200,v),Err(e)=>(e.status,json!({"error":e.message,"code":e.code}))};if let Ok(raw)=serde_json::to_vec(&body){let _=write!(stream,"HTTP/1.1 {status} Result\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",raw.len());let _=stream.write_all(&raw);}}));
    }
    for stream in listener.incoming() {
        let stream = stream?;
        if send.try_send(stream).is_err() {
            continue;
        }
    }
    drop(send);
    for thread in threads {
        let _ = thread.join();
    }
    drop(lock);
    Ok(())
}
