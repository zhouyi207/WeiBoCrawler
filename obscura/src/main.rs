use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::STANDARD};
use clap::Parser;
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    net::TcpStream,
    path::PathBuf,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tungstenite::{Message, WebSocket, stream::MaybeTlsStream};

#[derive(Parser)]
#[command(about = "使用 Obscura 获取知乎登录二维码，扫码确认后导出 cookies")]
struct Args {
    #[arg(long, default_value = ".tools/obscura.exe")]
    obscura: PathBuf,
    /// CDP 端口；默认 0，由系统选择空闲端口
    #[arg(long, default_value_t = 0)]
    port: u16,
    #[arg(long, default_value_t = 90)]
    timeout: u64,
    #[arg(long, default_value_t = 120)]
    /// 等待扫码确认的秒数；0 表示仅获取二维码
    hold: u64,
    #[arg(long, default_value = "output")]
    output: PathBuf,
}

struct Browser(Child);
impl Drop for Browser {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct Cdp {
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
    id: u64,
    session: Option<String>,
    events: Vec<Value>,
    trace: fs::File,
    started: Instant,
}
impl Cdp {
    fn trace(&mut self, entry: Value) {
        let row = json!({"elapsed_ms": self.started.elapsed().as_millis(), "event": entry});
        let _ = writeln!(self.trace, "{row}");
    }

    fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        let started = Instant::now();
        self.id += 1;
        let expected_id = self.id;
        let mut msg = json!({"id": expected_id, "method": method, "params": params});
        if let Some(s) = &self.session {
            msg["sessionId"] = json!(s);
        }
        self.socket.send(Message::Text(msg.to_string().into()))?;
        loop {
            let msg = self
                .socket
                .read()
                .with_context(|| format!("CDP {method} 读取失败"))?;
            if let Message::Text(text) = msg {
                let value: Value = serde_json::from_str(&text)?;
                if value["id"].as_u64() == Some(expected_id) {
                    self.trace(json!({"command": method, "duration_ms": started.elapsed().as_millis(), "error": value.get("error").is_some()}));
                    if let Some(error) = value.get("error") {
                        bail!("{method}: {error}");
                    }
                    return Ok(value["result"].clone());
                }
                if value["method"] == "Network.responseReceived" {
                    let response = &value["params"]["response"];
                    let url = response["url"].as_str().unwrap_or("");
                    let kind = if url.contains("/scan_info") {
                        "scan_status"
                    } else if url.contains("/login/") || url.ends_with("/sign_in") {
                        "login"
                    } else if url == "https://www.zhihu.com/robots.txt" {
                        "login_redirect"
                    } else {
                        "other"
                    };
                    if kind != "other" {
                        self.trace(json!({"network":kind, "status":response["status"], "browser_timestamp":value["params"]["timestamp"]}));
                    }
                }
                if self.events.len() < 2000 {
                    self.events.push(value);
                }
            }
        }
    }
    fn eval(&mut self, expression: &str) -> Result<Value> {
        let r = self.call(
            "Runtime.evaluate",
            json!({"expression": expression, "returnByValue": true, "awaitPromise": true}),
        )?;
        if r.get("exceptionDetails").is_some() {
            bail!("页面脚本错误: {}", r["exceptionDetails"]);
        }
        Ok(r["result"]["value"].clone())
    }
    fn screenshot(&mut self) -> Result<Vec<u8>> {
        let r = self.call("Page.captureScreenshot", json!({"format": "png"}))?;
        Ok(STANDARD.decode(r["data"].as_str().context("截图无数据")?)?)
    }

    fn zhihu_cookies(&mut self) -> Result<Vec<Value>> {
        // CDP includes HttpOnly cookies, unlike document.cookie. Obscura 0.2.3
        // returns the full jar, so explicitly filter domains and expiration.
        let response = self.call(
            "Network.getCookies",
            json!({"urls": ["https://www.zhihu.com/"]}),
        )?;
        let cookies = response["cookies"]
            .as_array()
            .context("CDP 未返回 cookies 数组")?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs_f64();
        Ok(cookies
            .iter()
            .filter(|cookie| valid_zhihu_cookie(cookie, now))
            .cloned()
            .collect())
    }
}

fn valid_zhihu_cookie(cookie: &Value, now: f64) -> bool {
    let domain = cookie["domain"]
        .as_str()
        .unwrap_or("")
        .trim_start_matches('.');
    let expires = cookie["expires"].as_f64().unwrap_or(-1.0);
    (domain == "zhihu.com" || domain.ends_with(".zhihu.com"))
        && (cookie["session"] == true || expires < 0.0 || expires > now)
}

fn has_login_cookie(cookies: &[Value]) -> bool {
    cookies.iter().any(|cookie| {
        cookie["name"] == "z_c0"
            && cookie["value"]
                .as_str()
                .is_some_and(|value| !value.is_empty())
    })
}

fn wait_for_login(cdp: &mut Cdp, args: &Args) -> Result<()> {
    if args.hold == 0 {
        println!("仅获取二维码；未等待登录，未导出 cookies。");
        return Ok(());
    }
    println!(
        "等待扫码登录（最多 {} 秒），请在知乎 App 中确认登录……",
        args.hold
    );
    let until = Instant::now() + Duration::from_secs(args.hold);
    while Instant::now() < until {
        // Obscura pumps timers autonomously. Runtime.evaluate here can wait
        // for a whole post-login navigation before allowing the cookie read.
        let cookies = cdp.zhihu_cookies()?;
        if has_login_cookie(&cookies) {
            let path = args.output.join("zhihu-cookies.json");
            let temporary = args.output.join("zhihu-cookies.json.tmp");
            fs::write(&temporary, serde_json::to_vec_pretty(&cookies)?)?;
            fs::rename(&temporary, &path).context("保存 cookies 文件失败")?;
            cdp.trace(json!({"cookies_saved": true, "count": cookies.len()}));
            println!(
                "已检测到登录凭据 z_c0，导出 {} 个 cookies（保留 HttpOnly 等属性）: {}",
                cookies.len(),
                path.canonicalize()?.display()
            );
            return Ok(());
        }
        thread::sleep(Duration::from_millis(250));
    }
    bail!("等待扫码确认超时，未检测到登录凭据；本次未导出 cookies，请重新运行后扫码并确认登录")
}

// Only accept an image that actually decodes to a QR code.
fn is_qr(bytes: &[u8]) -> bool {
    let Ok(img) = image::load_from_memory(bytes) else {
        return false;
    };
    let mut prepared = rqrr::PreparedImage::prepare(img.to_luma8());
    prepared
        .detect_grids()
        .iter_mut()
        .any(|grid| grid.decode().is_ok())
}

fn main() -> Result<()> {
    let args = Args::parse();
    anyhow::ensure!(args.timeout > 0, "--timeout 必须大于 0");
    // Ask the OS for a free port by default. An explicitly requested occupied
    // port still fails rather than attaching to an unrelated browser session.
    let port_check = std::net::TcpListener::bind(("127.0.0.1", args.port)).with_context(|| {
        format!(
            "无法绑定 CDP 端口 {}，可使用 --port 0 自动选择空闲端口",
            args.port
        )
    })?;
    let port = port_check.local_addr()?.port();
    fs::create_dir_all(&args.output)?;
    let previous = args.output.join("zhihu-login-qr.png");
    if previous.exists() {
        fs::remove_file(previous)?;
    }
    let log = fs::File::create(args.output.join("obscura.log"))?;
    let mut command = Command::new(&args.obscura);
    command
        .args(["serve", "--host", "127.0.0.1", "--port", &port.to_string()])
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    drop(port_check);
    eprintln!("使用本地 CDP 端口: {port}");
    let mut browser = Browser(
        command
            .spawn()
            .context("无法启动 Obscura，请先运行 scripts/install-obscura.ps1")?,
    );
    let endpoint = format!("ws://127.0.0.1:{port}/devtools/browser");
    let start = Instant::now();
    let socket = loop {
        if let Some(status) = browser.0.try_wait()? {
            bail!("Obscura 已退出: {status}，查看 output/obscura.log");
        }
        match tungstenite::connect(&endpoint) {
            Ok((socket, _)) => break socket,
            Err(e) if start.elapsed() > Duration::from_secs(15) => return Err(e.into()),
            Err(_) => thread::sleep(Duration::from_millis(200)),
        }
    };
    let mut cdp = Cdp {
        socket,
        id: 0,
        session: None,
        events: vec![],
        trace: fs::File::create(args.output.join("timing.jsonl"))?,
        started: Instant::now(),
    };
    if let MaybeTlsStream::Plain(stream) = cdp.socket.get_mut() {
        stream.set_read_timeout(Some(Duration::from_secs(args.timeout)))?;
        stream.set_write_timeout(Some(Duration::from_secs(15)))?;
    }
    let target = cdp.call("Target.createTarget", json!({"url": "about:blank"}))?;
    let attached = cdp.call(
        "Target.attachToTarget",
        json!({"targetId": target["targetId"], "flatten": true}),
    )?;
    cdp.session = Some(
        attached["sessionId"]
            .as_str()
            .context("缺少 sessionId")?
            .to_owned(),
    );
    cdp.call("Network.enable", json!({}))?;
    cdp.call("Runtime.enable", json!({}))?;
    eprintln!("正在打开知乎登录页……");
    // Zhihu's own nextUrl handling redirects after successful login. A small
    // same-origin text document avoids loading the expensive authenticated SPA.
    let navigation = cdp.call(
        "Page.navigate",
        json!({"url": "https://www.zhihu.com/signin?next=%2Frobots.txt"}),
    )?;
    if let Some(error) = navigation.get("errorText") {
        bail!("导航失败: {error}");
    }
    let deadline = Instant::now() + Duration::from_secs(args.timeout);
    let result: Result<()> = (|| {
        loop {
            let state = cdp.eval(include_str!("qr.js"))?;
            let mut bytes = None;
            if let Some(data) = state["data"]
                .as_str()
                .and_then(|s| s.strip_prefix("data:image/png;base64,"))
            {
                let candidate = STANDARD.decode(data)?;
                if is_qr(&candidate) {
                    bytes = Some(candidate);
                }
            }
            // Obscura 0.2.3 can leave Zhihu's canvas blank. The page's own
            // successful QR response carries the exact link encoded by that canvas.
            if bytes.is_none() {
                let id = cdp
                    .events
                    .iter()
                    .rev()
                    .find(|e| {
                        e["method"] == "Network.responseReceived"
                            && e["params"]["response"]["status"] == 200
                            && e["params"]["response"]["url"]
                                == "https://www.zhihu.com/api/v3/account/api/login/qrcode"
                    })
                    .and_then(|e| e["params"]["requestId"].as_str())
                    .map(str::to_owned);
                if let Some(id) = id {
                    let response = cdp.call("Network.getResponseBody", json!({"requestId": id}))?;
                    let body = response["body"].as_str().context("二维码响应无正文")?;
                    let body = if response["base64Encoded"] == true {
                        STANDARD.decode(body)?
                    } else {
                        body.as_bytes().to_vec()
                    };
                    let qr: Value = serde_json::from_slice(&body)?;
                    let link = qr["link"]
                        .as_str()
                        .filter(|s| !s.is_empty())
                        .context("二维码响应缺少 link")?;
                    let code = qrcode::QrCode::new(link.as_bytes())?;
                    let img = code
                        .render::<image::Luma<u8>>()
                        .min_dimensions(360, 360)
                        .build();
                    let mut buffer = std::io::Cursor::new(Vec::new());
                    img.write_to(&mut buffer, image::ImageFormat::Png)?;
                    let mut check = rqrr::PreparedImage::prepare(img);
                    let matches = check
                        .detect_grids()
                        .iter_mut()
                        .any(|g| g.decode().is_ok_and(|(_, text)| text == link));
                    anyhow::ensure!(matches, "生成二维码的解码校验失败");
                    eprintln!("已从页面二维码响应获取真实扫码链接，并通过解码校验。");
                    bytes = Some(buffer.into_inner());
                }
            }
            if bytes.is_none() && state["rect"].is_object() {
                let screenshot = cdp.screenshot()?;
                let img = image::load_from_memory(&screenshot)?;
                let rect = &state["rect"];
                let x = rect["x"].as_f64().unwrap_or(0.0).max(0.0) as u32;
                let y = rect["y"].as_f64().unwrap_or(0.0).max(0.0) as u32;
                let w = rect["width"].as_f64().unwrap_or(0.0).ceil() as u32;
                let h = rect["height"].as_f64().unwrap_or(0.0).ceil() as u32;
                if w > 0 && h > 0 && x + w <= img.width() && y + h <= img.height() {
                    let mut buffer = std::io::Cursor::new(Vec::new());
                    img.crop_imm(x, y, w, h)
                        .write_to(&mut buffer, image::ImageFormat::Png)?;
                    if is_qr(buffer.get_ref()) {
                        bytes = Some(buffer.into_inner());
                    }
                }
            }
            if let Some(bytes) = bytes {
                let path = args.output.join("zhihu-login-qr.png");
                fs::write(&path, bytes)?;
                cdp.trace(json!({"qr_saved": true}));
                println!("二维码已保存: {}", path.canonicalize()?.display());
                return wait_for_login(&mut cdp, &args);
            }
            if Instant::now() >= deadline {
                bail!(
                    "等待二维码超时：页面未生成可解码的二维码，诊断信息保存在 {}",
                    args.output.display()
                );
            }
            thread::sleep(Duration::from_secs(2));
        }
    })();
    if result.is_err() {
        if let Ok(html) = cdp.eval("document.documentElement.outerHTML") {
            fs::write(args.output.join("page.html"), html.as_str().unwrap_or(""))?;
        }
        if let Ok(png) = cdp.screenshot() {
            fs::write(args.output.join("page.png"), png)?;
        }
        fs::write(
            args.output.join("events.json"),
            serde_json::to_vec_pretty(&cdp.events)?,
        )?;
    }
    let _ = cdp.call(
        "Target.closeTarget",
        json!({"targetId": target["targetId"]}),
    );
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires .tools/obscura.exe; runs an actual CDP browser"]
    fn obscura_light_redirect_keeps_http_only_cookie() -> Result<()> {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0))?;
        let port = listener.local_addr()?.port();
        fs::create_dir_all("output/redirect-test")?;
        let mut command = Command::new(".tools/obscura.exe");
        command
            .args(["serve", "--host", "127.0.0.1", "--port", &port.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        drop(listener);
        let _browser = Browser(command.spawn()?);
        let started = Instant::now();
        let socket = loop {
            match tungstenite::connect(format!("ws://127.0.0.1:{port}/devtools/browser")) {
                Ok((socket, _)) => break socket,
                Err(e) if started.elapsed() > Duration::from_secs(10) => return Err(e.into()),
                Err(_) => thread::sleep(Duration::from_millis(100)),
            }
        };
        let mut cdp = Cdp {
            socket,
            id: 0,
            session: None,
            events: vec![],
            trace: fs::File::create("output/redirect-test/timing.jsonl")?,
            started: Instant::now(),
        };
        if let MaybeTlsStream::Plain(stream) = cdp.socket.get_mut() {
            stream.set_read_timeout(Some(Duration::from_secs(15)))?;
        }
        let target = cdp.call("Target.createTarget", json!({"url":"about:blank"}))?;
        let attached = cdp.call(
            "Target.attachToTarget",
            json!({"targetId":target["targetId"], "flatten":true}),
        )?;
        cdp.session = attached["sessionId"].as_str().map(str::to_owned);
        cdp.call("Network.setCookie", json!({"name":"obscura_test_cookie","value":"local-test-only","domain":".zhihu.com","path":"/","secure":true,"httpOnly":true}))?;

        // Simulate an asynchronous post-login redirect, not an explicit goto.
        cdp.call("Page.navigate", json!({"url":"data:text/html,<script>setTimeout(()=>location.href='https://www.zhihu.com/robots.txt',100)</script>"}))?;
        let started = Instant::now();
        loop {
            let cookies = cdp.zhihu_cookies()?;
            assert!(
                cookies
                    .iter()
                    .any(|c| c["name"] == "obscura_test_cookie" && c["httpOnly"] == true)
            );
            if cdp.eval("location.href")? == "https://www.zhihu.com/robots.txt" {
                break;
            }
            anyhow::ensure!(
                started.elapsed() < Duration::from_secs(5),
                "lightweight redirect did not complete"
            );
            thread::sleep(Duration::from_millis(100));
        }

        let cookies = cdp.zhihu_cookies()?;
        assert!(
            cookies
                .iter()
                .any(|c| c["name"] == "obscura_test_cookie" && c["httpOnly"] == true)
        );
        eprintln!(
            "redirect and cookie read completed in {} ms",
            started.elapsed().as_millis()
        );
        Ok(())
    }

    #[test]
    fn filters_foreign_and_expired_cookies() {
        assert!(valid_zhihu_cookie(
            &json!({"domain": ".zhihu.com", "expires": -1}),
            100.0
        ));
        assert!(valid_zhihu_cookie(
            &json!({"domain": "www.zhihu.com", "expires": 200}),
            100.0
        ));
        assert!(!valid_zhihu_cookie(
            &json!({"domain": "evilzhihu.com", "expires": -1}),
            100.0
        ));
        assert!(!valid_zhihu_cookie(
            &json!({"domain": "zhihu.com.evil.test", "expires": -1}),
            100.0
        ));
        assert!(!valid_zhihu_cookie(
            &json!({"domain": ".zhihu.com", "expires": 100}),
            100.0
        ));
    }

    #[test]
    fn anonymous_cookies_do_not_count_as_login() {
        assert!(!has_login_cookie(&[
            json!({"name": "_xsrf", "value": "test"})
        ]));
        assert!(!has_login_cookie(&[json!({"name": "z_c0", "value": ""})]));
        assert!(has_login_cookie(&[
            json!({"name": "z_c0", "value": "test-only", "httpOnly": true})
        ]));
    }
}
