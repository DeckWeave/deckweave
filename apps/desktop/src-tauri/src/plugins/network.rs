use super::catalog;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, SyncSender, sync_channel},
    },
    thread::{self, JoinHandle},
    time::Duration,
};
pub type Socket = tungstenite::WebSocket<TcpStream>;
pub struct Peer {
    pub role: String,
    pub uuid: String,
    pub socket: Socket,
}
#[derive(Clone)]
pub struct Route {
    pub uuid: String,
    pub nonce: String,
    pub role: String,
    pub root: PathBuf,
    pub page: String,
    pub info: Value,
    pub action_info: Option<Value>,
    pub sender: SyncSender<Peer>,
    pub errors: Option<SyncSender<String>>,
}
pub struct Network {
    pub port: u16,
    routes: Arc<Mutex<BTreeMap<String, Route>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}
impl Network {
    pub fn start() -> Result<Self, String> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|e| e.to_string())?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        let routes = Arc::new(Mutex::new(BTreeMap::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (thread_routes, thread_stop) = (routes.clone(), stop.clone());
        let active = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let thread = thread::spawn(move || {
            while !thread_stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        if active.load(Ordering::Relaxed) >= 32 {
                            continue;
                        }
                        active.fetch_add(1, Ordering::Relaxed);
                        let active = active.clone();
                        let routes = thread_routes.clone();
                        thread::spawn(move || {
                            let outcome = serve(stream, &routes, port);
                            #[cfg(test)]
                            if let Err(error) = outcome {
                                eprintln!("Plugin connection: {error}");
                            }
                            #[cfg(not(test))]
                            let _ = outcome;
                            active.fetch_sub(1, Ordering::Relaxed);
                        });
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(20))
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            port,
            routes,
            stop,
            thread: Some(thread),
        })
    }
    pub fn channel() -> (SyncSender<Peer>, Receiver<Peer>) {
        sync_channel(4)
    }
    pub fn add(&self, route: Route) {
        if let Ok(mut routes) = self.routes.lock() {
            routes.insert(route.uuid.clone(), route);
        }
    }
    pub fn remove(&self, uuid: &str) {
        if let Ok(mut routes) = self.routes.lock() {
            routes.remove(uuid);
        }
    }
    pub fn url(&self, nonce: &str, page: &str) -> String {
        format!("http://127.0.0.1:{}/assets/{}/{}", self.port, nonce, page)
    }
}
impl Drop for Network {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
fn serve(
    mut stream: TcpStream,
    routes: &Mutex<BTreeMap<String, Route>>,
    port: u16,
) -> Result<(), String> {
    // Windows can inherit the listener's nonblocking mode on accepted sockets.
    // Complete the bounded HTTP/registration handshake before switching to polling.
    stream.set_nonblocking(false).map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;
    let mut buffer = [0; 16384];
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    let count = loop {
        let count = stream.peek(&mut buffer).map_err(|e| e.to_string())?;
        if buffer[..count].windows(4).any(|s| s == b"\r\n\r\n") {
            break count;
        }
        if count == 0 || count == buffer.len() || std::time::Instant::now() >= deadline {
            return Err("Incomplete HTTP header".into());
        }
        thread::sleep(Duration::from_millis(5));
    };
    let head = String::from_utf8_lossy(&buffer[..count]);
    if head.to_ascii_lowercase().contains("upgrade: websocket") {
        let config = tungstenite::protocol::WebSocketConfig::default()
            .max_message_size(Some(3 * 1024 * 1024))
            .max_frame_size(Some(3 * 1024 * 1024));
        let mut socket =
            tungstenite::accept_with_config(stream, Some(config)).map_err(|e| e.to_string())?;
        let message = socket.read().map_err(|e| e.to_string())?;
        let value: Value = serde_json::from_str(message.to_text().map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        let uuid = value["uuid"].as_str().ok_or("Missing registration UUID")?;
        let route = routes
            .lock()
            .map_err(|e| e.to_string())?
            .get(uuid)
            .cloned()
            .ok_or("Unknown registration UUID")?;
        if value["event"] != route.role {
            return Err("Registration role mismatch".into());
        }
        socket
            .get_mut()
            .set_nonblocking(true)
            .map_err(|e| e.to_string())?;
        route
            .sender
            .try_send(Peer {
                role: route.role,
                uuid: uuid.into(),
                socket,
            })
            .map_err(|_| "Registration queue full".into())
    } else {
        let count = stream.read(&mut buffer).map_err(|e| e.to_string())?;
        let head = String::from_utf8_lossy(&buffer[..count]);
        if let Some(path) = head
            .lines()
            .next()
            .and_then(|line| line.strip_prefix("POST "))
            .and_then(|line| line.split_whitespace().next())
        {
            let nonce = path
                .strip_prefix("/errors/")
                .ok_or("Unknown diagnostic route")?;
            let route = routes
                .lock()
                .map_err(|e| e.to_string())?
                .values()
                .find(|r| r.nonce == nonce)
                .cloned()
                .ok_or("Unknown diagnostic session")?;
            let content_length = head
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .and_then(|n| n.trim().parse::<usize>().ok())
                })
                .unwrap_or(0);
            if content_length > 4096 {
                return Err("Diagnostic too large".into());
            }
            let offset = buffer[..count]
                .windows(4)
                .position(|w| w == b"\r\n\r\n")
                .ok_or("Missing HTTP header")?
                + 4;
            let mut body = buffer[offset..count].to_vec();
            if body.len() < content_length {
                let remaining = content_length - body.len();
                let start = body.len();
                body.resize(content_length, 0);
                stream
                    .read_exact(&mut body[start..start + remaining])
                    .map_err(|e| e.to_string())?;
            }
            if let Some(sender) = route.errors {
                let message = String::from_utf8_lossy(&body[..content_length])
                    .chars()
                    .take(512)
                    .collect();
                let _ = sender.try_send(message);
            }
            stream
                .write_all(b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n")
                .map_err(|e| e.to_string())?;
            return Ok(());
        }
        let path = head
            .lines()
            .next()
            .and_then(|line| line.strip_prefix("GET "))
            .and_then(|line| line.split_whitespace().next())
            .ok_or("Only GET is supported")?;
        let path = path.split('?').next().unwrap_or(path);
        let path = percent_decode(path)?;
        let (nonce, relative) = path
            .strip_prefix("/assets/")
            .and_then(|s| s.split_once('/'))
            .ok_or("Unknown asset URL")?;
        let route = routes
            .lock()
            .map_err(|e| e.to_string())?
            .values()
            .find(|r| r.nonce == nonce)
            .cloned()
            .ok_or("Unknown asset session")?;
        let file = catalog::resolve(&route.root, relative)?;
        let mut bytes = std::fs::read(&file).map_err(|e| e.to_string())?;
        if bytes.len() > 16 * 1024 * 1024 {
            return Err("Asset too large".into());
        }
        let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
        let mime = match ext {
            "html" | "htm" => "text/html; charset=utf-8",
            "js" | "mjs" => "application/javascript",
            "css" => "text/css",
            "json" => "application/json",
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "svg" => "image/svg+xml",
            "woff2" => "font/woff2",
            "ttf" => "font/ttf",
            _ => "application/octet-stream",
        };
        if relative == route.page && matches!(ext, "html" | "htm") {
            if route.errors.is_some() {
                let report = format!(
                    "<script>window.addEventListener('error',e=>{{fetch('/errors/{}',{{method:'POST',body:'JavaScript: '+e.message}}).catch(()=>{{}});}});window.addEventListener('unhandledrejection',e=>{{fetch('/errors/{}',{{method:'POST',body:'Promise: '+String(e.reason)}}).catch(()=>{{}});}});</script>",
                    route.nonce, route.nonce
                );
                let html = String::from_utf8_lossy(&bytes);
                let position = html.to_ascii_lowercase().find("<script").unwrap_or(0);
                let mut next = html[..position].as_bytes().to_vec();
                next.extend(report.as_bytes());
                next.extend(html[position..].as_bytes());
                bytes = next;
            }
            let mut args = vec![
                json!(port.to_string()),
                json!(route.uuid),
                json!(route.role),
                json!(route.info.to_string()),
            ];
            if let Some(info) = route.action_info {
                args.push(json!(info.to_string()));
            }
            let bootstrap = format!(
                "<script>window.addEventListener('load',()=>{{if(typeof connectElgatoStreamDeckSocket==='function')connectElgatoStreamDeckSocket(...{});}});</script>",
                serde_json::to_string(&args)
                    .map_err(|e| e.to_string())?
                    .replace('<', "\\u003c")
            );
            bytes.extend(bootstrap.as_bytes());
        }
        write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",bytes.len()).map_err(|e|e.to_string())?;
        stream.write_all(&bytes).map_err(|e| e.to_string())
    }
}
fn percent_decode(text: &str) -> Result<String, String> {
    let mut bytes = vec![];
    let mut chars = text.bytes();
    while let Some(c) = chars.next() {
        if c == b'%' {
            let a = chars.next().ok_or("Bad URL")?;
            let b = chars.next().ok_or("Bad URL")?;
            let s = String::from_utf8(vec![a, b]).map_err(|_| "Bad URL")?;
            bytes.push(u8::from_str_radix(&s, 16).map_err(|_| "Bad URL")?);
        } else {
            bytes.push(c);
        }
    }
    String::from_utf8(bytes).map_err(|_| "Bad URL".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn obs_plugin_authenticates_and_reports_version_over_both_real_websockets() {
        use base64::{Engine, engine::general_purpose::STANDARD};
        use sha2::{Digest, Sha256};
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let obs_port = listener.local_addr().unwrap().port();
        let server = thread::spawn(move || {
            let deadline = std::time::Instant::now() + Duration::from_secs(8);
            let stream = loop {
                match listener.accept() {
                    Ok((s, _)) => break s,
                    Err(_) if std::time::Instant::now() < deadline => {
                        thread::sleep(Duration::from_millis(10))
                    }
                    Err(e) => panic!("OBS accept: {e}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut socket = tungstenite::accept(stream).unwrap();
            socket.send(tungstenite::Message::Text(json!({"op":0,"d":{"rpcVersion":1,"obsWebSocketVersion":"5.6.3","authentication":{"salt":"salt","challenge":"challenge"}}}).to_string().into())).unwrap();
            let identify: Value =
                serde_json::from_str(socket.read().unwrap().to_text().unwrap()).unwrap();
            let secret = STANDARD.encode(Sha256::digest(b"smoke-passwordsalt"));
            let expected = STANDARD.encode(Sha256::digest(format!("{secret}challenge").as_bytes()));
            assert_eq!(identify["op"], 1);
            assert_eq!(identify["d"]["authentication"], expected);
            socket
                .send(tungstenite::Message::Text(
                    json!({"op":2,"d":{"negotiatedRpcVersion":1}})
                        .to_string()
                        .into(),
                ))
                .unwrap();
            let request: Value =
                serde_json::from_str(socket.read().unwrap().to_text().unwrap()).unwrap();
            assert_eq!(request["d"]["requestType"], "GetVersion");
            socket.send(tungstenite::Message::Text(json!({"op":7,"d":{"requestType":"GetVersion","requestId":request["d"]["requestId"],"requestStatus":{"result":true,"code":100},"responseData":{"obsVersion":"32.fixture"}}}).to_string().into())).unwrap();
            thread::sleep(Duration::from_millis(400));
        });
        let net = Network::start().unwrap();
        let (tx, rx) = Network::channel();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../plugins/dev.deckweave.obs.sdPlugin")
            .canonicalize()
            .unwrap();
        net.add(Route {
            uuid: "obs-test-token".into(),
            nonce: "obs-test-nonce".into(),
            role: "registerPlugin".into(),
            root: root.clone(),
            page: "backend.mjs".into(),
            info: json!({}),
            action_info: None,
            sender: tx,
            errors: None,
        });
        let mut command = std::process::Command::new("node");
        command.arg(root.join("backend.mjs")).args([
            "-port",
            &net.port.to_string(),
            "-pluginUUID",
            "obs-test-token",
            "-registerEvent",
            "registerPlugin",
            "-info",
            "{}",
        ]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command.spawn().unwrap();
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut peer = rx.recv_timeout(Duration::from_secs(5)).unwrap();
            peer.socket.get_mut().set_nonblocking(false).unwrap();
            peer.socket
                .get_mut()
                .set_read_timeout(Some(Duration::from_secs(6)))
                .unwrap();
            peer.socket.send(tungstenite::Message::Text(json!({"event":"willAppear","action":"dev.deckweave.obs.status","context":"obs-button","payload":{"settings":{"port":obs_port,"password":"smoke-password"}}}).to_string().into())).unwrap();
            let mut found = false;
            for _ in 0..8 {
                let message: Value =
                    serde_json::from_str(peer.socket.read().unwrap().to_text().unwrap()).unwrap();
                if message["event"] == "setTitle" && message["payload"]["title"] == "OBS 32.fixture"
                {
                    found = true;
                    break;
                }
            }
            assert!(found, "OBS version must reach Stream Deck feedback");
        }));
        let _ = child.kill();
        let _ = child.wait();
        server.join().unwrap();
        outcome.unwrap();
    }
    #[test]
    fn real_node_plugin_registers_receives_input_and_changes_feedback() {
        let net = Network::start().unwrap();
        let (tx, rx) = Network::channel();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../plugins/dev.deckweave.diagnostics.sdPlugin")
            .canonicalize()
            .unwrap();
        net.add(Route {
            uuid: "test-token".into(),
            nonce: "test-nonce".into(),
            role: "registerPlugin".into(),
            root: root.clone(),
            page: "backend.mjs".into(),
            info: json!({}),
            action_info: None,
            sender: tx,
            errors: None,
        });
        let mut command = std::process::Command::new("node");
        command.arg(root.join("backend.mjs")).args([
            "-port",
            &net.port.to_string(),
            "-pluginUUID",
            "test-token",
            "-registerEvent",
            "registerPlugin",
            "-info",
            "{}",
        ]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command.spawn().expect("Node is a project prerequisite");
        let result = || {
            let mut peer = rx.recv_timeout(Duration::from_secs(5)).unwrap();
            assert_eq!(peer.role, "registerPlugin");
            peer.socket.get_mut().set_nonblocking(false).unwrap();
            peer.socket
                .get_mut()
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            peer.socket.send(tungstenite::Message::Text(json!({"event":"willAppear","context":"button-1","payload":{"settings":{"prefix":"Saved"}}}).to_string().into())).unwrap();
            let title: Value =
                serde_json::from_str(peer.socket.read().unwrap().to_text().unwrap()).unwrap();
            assert_eq!(title["payload"]["title"], "Saved 0");
            let _ = peer.socket.read().unwrap();
            peer.socket
                .send(tungstenite::Message::Text(
                    json!({"event":"keyUp","context":"button-1","payload":{}})
                        .to_string()
                        .into(),
                ))
                .unwrap();
            let title: Value =
                serde_json::from_str(peer.socket.read().unwrap().to_text().unwrap()).unwrap();
            assert_eq!(title["payload"]["title"], "Saved 1");
        };
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(result));
        let _ = child.kill();
        let _ = child.wait();
        outcome.unwrap();
    }
    #[test]
    fn native_panel_receives_unknown_y_drag_and_independent_dial_and_emits_renderable_svg() {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        let net = Network::start().unwrap();
        let (tx, rx) = Network::channel();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../plugins/dev.deckweave.panel.sdPlugin")
            .canonicalize()
            .unwrap();
        net.add(Route {
            uuid: "panel-token".into(),
            nonce: "panel-nonce".into(),
            role: "registerPlugin".into(),
            root: root.clone(),
            page: "plugin.mjs".into(),
            info: json!({}),
            action_info: None,
            sender: tx,
            errors: None,
        });
        let mut command = std::process::Command::new("node");
        command.arg(root.join("plugin.mjs")).args([
            "-port",
            &net.port.to_string(),
            "-pluginUUID",
            "panel-token",
            "-registerEvent",
            "registerPlugin",
            "-info",
            "{}",
        ]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command.spawn().unwrap();
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut peer = rx.recv_timeout(Duration::from_secs(5)).unwrap();
            peer.socket.get_mut().set_nonblocking(false).unwrap();
            peer.socket
                .get_mut()
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut roundtrip = |event: &str, payload: Value| {
                peer.socket
                    .send(tungstenite::Message::Text(
                        json!({"event":event,"context":"widget","payload":payload})
                            .to_string()
                            .into(),
                    ))
                    .unwrap();
                let image: Value =
                    serde_json::from_str(peer.socket.read().unwrap().to_text().unwrap()).unwrap();
                let title: Value =
                    serde_json::from_str(peer.socket.read().unwrap().to_text().unwrap()).unwrap();
                (
                    image["payload"]["image"].as_str().unwrap().to_owned(),
                    title["payload"]["title"].as_str().unwrap().to_owned(),
                )
            };
            assert_eq!(
                roundtrip("willAppear", json!({"canvas":[800,128]})).1,
                "Panel 50%"
            );
            let (image, title) = roundtrip(
                "panelMove",
                json!({"position":{"x":0.8,"y":null},"canvas":[800,128]}),
            );
            assert_eq!(title, "Panel 80%");
            let svg = String::from_utf8(STANDARD.decode(image.split_once(',').unwrap().1).unwrap())
                .unwrap();
            assert!(svg.contains("Y: unknown"));
            assert!(
                deckweave_n4::Renderer::new()
                    .unwrap()
                    .image_data_jpeg(&image)
                    .is_ok()
            );
            assert_eq!(
                roundtrip(
                    "dialRotate",
                    json!({"ticks":-3,"control":"dial-2","canvas":[800,128]})
                )
                .1,
                "Panel 77%"
            );
            let (image, _) = roundtrip(
                "dialRotate",
                json!({"ticks":0,"control":"dial-2","canvas":[200,128]}),
            );
            let svg = String::from_utf8(STANDARD.decode(image.split_once(',').unwrap().1).unwrap())
                .unwrap();
            assert!(svg.contains("viewBox=\"0 0 200 128\""));
            assert!(!svg.contains("viewBox=\"0 0 800 128\""));
            assert!(
                deckweave_n4::Renderer::new()
                    .unwrap()
                    .image_data_png(&image, [200, 128])
                    .is_ok()
            );
        }));
        let _ = child.kill();
        let _ = child.wait();
        outcome.unwrap();
    }
    #[test]
    fn asset_server_bootstraps_only_authorized_inspector_and_rejects_traversal() {
        let net = Network::start().unwrap();
        let (tx, _rx) = Network::channel();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../plugins/dev.deckweave.diagnostics.sdPlugin");
        net.add(Route {
            uuid: "button-1".into(),
            nonce: "secret".into(),
            role: "registerPropertyInspector".into(),
            root,
            page: "settings.html".into(),
            info: json!({}),
            action_info: Some(json!({"payload":{"settings":{"prefix":"</script>"}}})),
            sender: tx,
            errors: None,
        });
        let get = |path: &str| {
            let mut s = TcpStream::connect(("127.0.0.1", net.port)).unwrap();
            s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            write!(s, "GET {path} HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
            let mut result = String::new();
            let _ = s.read_to_string(&mut result);
            result
        };
        let page = get("/assets/secret/settings.html");
        assert!(page.starts_with("HTTP/1.1 200"));
        assert!(page.contains("registerPropertyInspector"));
        assert!(page.contains("\\u003c/script>"));
        assert!(!get("/assets/wrong/settings.html").contains("HTTP/1.1 200"));
        assert!(!get("/assets/secret/%2e%2e/Cargo.toml").contains("HTTP/1.1 200"));
    }
}
