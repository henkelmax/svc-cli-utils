use std::io;
use std::io::{Cursor, Error, Read, Write};
use std::net::SocketAddr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio;
use console::style;
use tokio::net::{lookup_host, UdpSocket};
use url::Url;
use uuid::{Uuid, uuid};
use crate::PortArgs;
use mc_varint::{VarInt, VarIntWrite};
use tokio::time::timeout;

const MAGIC_BYTE: u8 = 0b11111111;
const CHECK_V1: Uuid = uuid!("58bc9ae9-c7a8-45e4-a11c-efbb67199425");
const DEFAULT_PORT: u16 = 24454;

#[tokio::main]
pub async fn port_command(args: PortArgs) {
    let voice_chat_url = format!("voicechat://{}", args.url);

    let url_result = Url::parse(voice_chat_url.as_str());

    if let Err(ref e) = url_result {
        eprintln!("{}", style(format!("Failed to parse voice chat URL: {}", e.to_string())).red());
        return;
    }

    let url = url_result.unwrap();

    if url.host_str().is_none() {
        eprintln!("{}", style("No host provided").red());
        return;
    }
    let host = url.host_str().unwrap();

    let ip_result = lookup(host).await;

    if let Err(ref e) = ip_result {
        eprintln!("{}", style(format!("Failed to look up host {}: {}", host, e.to_string())).red());
        return;
    }

    let mut socket_addr = ip_result.unwrap();

    let port = url.port().unwrap_or(DEFAULT_PORT);
    socket_addr.set_port(port);

    let mut ip = socket_addr.ip().to_string();
    if socket_addr.is_ipv6() {
        ip = format!("[{}]", ip);
    }

    if !host.eq(&ip) {
        println!("Resolved host to {}", ip);
    }

    println!("Sending pings to {}:", socket_addr.to_string());

    let attempts = args.attempts.unwrap_or(10);

    for i in 1..=attempts {
        let ping = Ping { id: Uuid::new_v4(), timestamp: current_millis() };

        println!("Pinging... ({}/{})", i, attempts);

        let ping_result = send_ping(socket_addr, ping).await;

        if let Err(ref e) = ping_result {
            eprintln!("{}", style(format!("Failed to ping {}:{}: {}", host, port, e.to_string())).red());
            return;
        }

        match ping_result.unwrap() {
            PingResult::Success(ping) => {
                println!("{}", style(format!("Got a response in {}ms after {} attempt(s)", ping, i)).green());
                return;
            }
            _ => {}
        }
    }

    println!("{}", style(format!("Timed out after {} attempt(s)", attempts)).yellow());
}

async fn lookup(host: &str) -> io::Result<SocketAddr> {
    let mut addresses = lookup_host(format!("{}:0", host)).await?;

    let mut first_v6: Option<SocketAddr> = None;

    while let Some(address) = addresses.next() {
        match address {
            SocketAddr::V4(_) => return Ok(address),
            SocketAddr::V6(_) => first_v6 = Some(address)
        }
    }

    if first_v6.is_some() {
        return Ok(first_v6.unwrap());
    }

    return Err(Error::new(
        io::ErrorKind::AddrNotAvailable,
        "no resolvable addresses",
    ));
}

async fn send_ping(socket_addr: SocketAddr, ping: Ping) -> io::Result<PingResult> {
    let socket;
    if socket_addr.is_ipv6() {
        socket = UdpSocket::bind("[::]:0").await?;
    } else {
        socket = UdpSocket::bind("0.0.0.0:0").await?;
    }

    let mut buffer = Cursor::new(Vec::with_capacity(1 + 16 + 1 + 24));

    buffer.write(&[MAGIC_BYTE])?;
    buffer.write(CHECK_V1.as_bytes())?;

    let ping_bytes = ping.to_bytes();
    let data_length = VarInt::from(ping_bytes.len() as i32);
    buffer.write_var_int(data_length)?;
    buffer.write(&ping_bytes)?;
    socket.send_to(&buffer.into_inner(), &socket_addr.to_string()).await?;

    let mut recv_buffer = [0; 1024];

    let timeout_result = timeout(Duration::from_secs(1), socket.recv_from(&mut recv_buffer)).await;

    if let Err(_) = timeout_result {
        return Ok(PingResult::Timeout);
    }

    let (bytes_received, _) = timeout_result.unwrap()?;

    let received_data = &recv_buffer[..bytes_received];

    let pong = Ping::from_bytes(received_data)?;

    return Ok(PingResult::Success(current_millis() - pong.timestamp));
}

fn current_millis() -> i64 {
    let current_time = SystemTime::now();
    let duration_since_epoch = match current_time.duration_since(UNIX_EPOCH) {
        Ok(duration) => duration,
        Err(_) => return 0,
    };

    return duration_since_epoch.as_secs() as i64 * 1000 + duration_since_epoch.subsec_millis() as i64;
}

enum PingResult {
    Success(i64),
    Timeout,
}

struct Ping {
    id: Uuid,
    timestamp: i64,
}

impl Ping {
    fn to_bytes(&self) -> Vec<u8> {
        let mut buffer = Vec::new();
        buffer.extend_from_slice(self.id.as_bytes());
        buffer.extend_from_slice(&self.timestamp.to_be_bytes());
        return buffer;
    }

    fn from_bytes(data: &[u8]) -> io::Result<Ping> {
        let mut cursor = Cursor::new(data);

        let mut uuid_bytes = [0; 16];
        cursor.read_exact(&mut uuid_bytes)?;
        let id = Uuid::from_bytes(uuid_bytes);

        let mut timestamp_bytes = [0; 8];
        cursor.read_exact(&mut timestamp_bytes)?;
        let timestamp = i64::from_be_bytes(timestamp_bytes);

        return Ok(Ping { id, timestamp });
    }
}
