# 知乎登录二维码 · Rust + Obscura

Rust 程序启动本地 Obscura，通过 CDP 打开 `https://www.zhihu.com/signin?next=%2Frobots.txt`，保存真实登录二维码到 `output/zhihu-login-qr.png`。扫码并在 App 中确认登录后，自动导出 cookies 到 `output/zhihu-cookies.json`。

## 运行（Windows x64）

需要 Rust 工具链。当前目录已下载 Obscura；新环境先安装：

```powershell
powershell -ExecutionPolicy Bypass -File scripts/install-obscura.ps1
```

在本目录执行：

```powershell
cargo run --release
```

默认由系统选择空闲 CDP 端口，不再固定使用 19222。也可通过 `--port 19223` 固定端口；指定端口被占用时会报错，使用 `--port 0` 恢复自动选择。已有扫码会话不会被终止。同时运行多个会话时，请通过 `--output` 指定不同目录，避免二维码和日志互相覆盖。

看到“二维码已保存”后打开 `output/zhihu-login-qr.png`，使用知乎 App 扫码并确认登录。默认等待最多 120 秒；检测到登录凭据 `z_c0` 后导出 cookies 并关闭浏览器。二维码有有效期，过期后重新运行；延长等待时间不会延长二维码有效期。

`output/zhihu-cookies.json` 是 CDP cookie 对象数组，保留 `name`、`value`、`domain`、`path`、`expires`、`httpOnly`、`secure`、`sameSite` 等返回属性，可供后续程序读取或导入浏览器。导出仅包含知乎域的未过期 cookies。登录判断依据是新会话收到非空 `z_c0`，不额外验证用户资料接口。

超时以非零状态退出，本次不导出匿名 cookies；以前成功导出的 cookies 文件会保留，只有本次成功登录才覆盖，请以本次成功日志为准。`--hold 0` 仅获取二维码，不导出 cookies。cookies 文件包含登录凭据，输出目录已加入 `.gitignore`。

```powershell
# 自定义等待时间、会话保持时间及输出目录
cargo run --release -- --timeout 90 --hold 120 --output output

# 仅验证抓取，不等待扫码
cargo run -- --hold 0

# 更换二进制位置或 CDP 端口
cargo run -- --obscura .tools/obscura.exe --port 19223
```

## 实现与验证

为减少确认登录后的等待，使用知乎原生 `next` 参数跳转到同站的 `robots.txt`，避免登录后加载首页应用。等待期间直接查询 cookies，不执行可能等待整个导航完成的 `Runtime.evaluate`；每次查询完成后间隔 250 毫秒。知乎页面自身的扫码轮询保持不变，因此不承诺手机确认后 250 毫秒内完成。

`output/timing.jsonl` 在成功和失败时均记录 CDP 命令耗时、二维码保存、扫码/登录响应、轻量跳转和 cookies 导出时间。时间为本次 CDP 连接建立后的相对毫秒数；网络事件同时保留浏览器时间戳。日志不记录 cookie 值、扫码 token 或响应正文。若仍有延迟，可通过这份文件区分网络等待和 CDP 阻塞。

使用真实 Obscura 的轻量跳转回归测试（含 HttpOnly 测试 cookie 保留校验）：

```powershell
cargo test obscura_light_redirect_keeps_http_only_cookie -- --ignored --nocapture
```

此测试验证跳转和 cookies 读取路径；完整的手机确认到凭据导出耗时仍需实际扫码测量。

- 使用官方 Obscura v0.2.3 Windows 渲染版本，不依赖 Chrome、Python 或 Node.js。
- 优先导出页面二维码 Canvas。实测此版本能收到知乎的二维码接口响应并轮询扫码状态，但 Canvas 为空白，因此读取同一会话的 `POST /api/v3/account/api/login/qrcode` 响应中的 `link`，用 Rust `qrcode` 生成 PNG。
- 使用 `rqrr` 解码校验生成图片，确认内容与接口返回链接完全一致。失败时不会把空白图片作为成功结果。
- 超时保存 `output/page.html`、`output/page.png`、`output/events.json`，浏览器日志为 `output/obscura.log`。这些文件可能包含临时会话信息，输出目录已加入 `.gitignore`。
- 程序退出时清理自身启动的浏览器进程。重复运行前会删除旧二维码，避免误用过期图片。
- 使用 CDP `Network.getCookies` 读取同一会话的 cookies（包括 HttpOnly），不使用 `document.cookie`。过滤无关域及过期项，发现 `z_c0` 后写入临时文件并替换正式 cookies 文件。

已通过真实知乎页面抓取和解码校验、`cargo test`（域名/过期过滤、匿名会话判定），以及 `cargo clippy --all-targets -- -D warnings`。实际账号登录和 cookies 导出需要用户扫码确认。

上游文档与下载：[Obscura](https://github.com/h4ckf0r0day/obscura)、[v0.2.3](https://github.com/h4ckf0r0day/obscura/releases/tag/v0.2.3)。
