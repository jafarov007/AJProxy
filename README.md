# AJProxy

<p align="center">
  <img src="banner.png" alt="AJProxy Banner" width="100%">
</p>

AJProxy is an ultra-fast, lightweight, and professional HTTP/HTTPS interception proxy and security testing tool built from the ground up in Rust (**7,400+ lines of pure Rust code**). Leveraging Rust's memory safety, concurrency model, and high performance, AJProxy delivers sub-millisecond packet interception with a minimal system memory footprint. 

It features a modern graphical user interface powered by `egui`, enabling security researchers and ethical hackers to intercept, inspect, and modify web & WebSocket traffic in real time.

---

## 🌟 Key Features

### ⚡ RFC 6455 WebSocket Suite
- **WS History**: Live WebSocket traffic viewer with connection session tree, directional stream table (**⬆️ Client** / **⬇️ Server**), colored Opcode badges (`TEXT`, `BINARY`, `PING`, `PONG`, `CLOSE`), and multi-mode inspector (**Raw Text**, **Formatted Hex Dump**, **JSON**).
- **WS Intercept**: Pause live incoming and outgoing WebSocket frames on the fly, edit payloads and Opcodes, and issue **▶️ Forward** or **✖ Drop** actions.
- **WS Repeater Workbench**: Multi-tabbed interactive testing workbench for custom WebSocket endpoints (`wss://` and `ws://`) with custom frame generation and real-time response event logging.
- **Protocol Features**: Full support for RFC 6455 4-byte XOR masking/unmasking, 2-byte Close status code parsing (e.g., `1000 Normal`, `1001 Going Away`, `1006 Abnormal`), auto Ping/Pong mirroring, and fragmented frame reassembly (`FIN=0` + `Continuation 0x0`).

### 🛡️ Real-Time HTTP/HTTPS Interception & Header Engine
- **Send to Repeater**: One-click transfer from traffic history to the interactive Repeater workbench.
- **Custom Header & Key Auto-Injection**: Define custom header rules (e.g. `X-Bounty-Key: secret123`, `X-Forwarded-For: 127.0.0.1`) with target domain scoping (`*target*` or `*` global). Automatically injects headers into all HTTP requests and WebSocket handshakes (`Upgrade: websocket`).
- **Advanced Traffic & Noise Filtering**:
  - **Host Filter Modal**: Wildcard & substring domain filtering (e.g., `*target*`, `api.example.com`) to isolate target application scope.
  - **Zero-Byte Suppression**: One-click toggle to suppress 0-length responses (e.g., 204 No Content, empty preflight packets).
  - **Pre-Configured Asset Filtering**: Hides `.css`, `.js`, `.png`, `.jpg`, `.gif`, `.svg`, `.ico`, `.woff2`, Cloudflare challenges, and telemetry.

### 🔑 Cross-Platform Certificate Trust Engine (Linux / macOS / Windows)
- **Modular OS Installers** (`linux.rs`, `windows.rs`, `macos.rs`): Automatically generates 365-day compliant Root CA certificates with one-click automatic CA trust installation and cleanup for system/browser trust stores across Linux, Windows, and macOS.
- **Direct TCP Media Passthrough**: Zero-overhead passthrough for video streaming CDNs (`googlevideo.com`, `gvt1.com`, `ytimg.com`) ensuring 4K video streaming without buffering.

### 🛠️ Security Testing Toolkit
- Integrated modules: **Dashboard**, **HTTP Traffic History**, **HTTP Repeater**, **WebSocket Suite**, **Sitemap**, **Comparer**, **Decoder**, **Intruder/Bruteforce**, and **Settings**.

---

## ⚖️ Legal & Educational Disclaimer

> **IMPORTANT**: AJProxy is strictly intended for **authorized penetration testing, ethical security research, and educational purposes only**. You may only use this tool against computer systems, web applications, and networks for which you have explicit, documented authorization from the owner.
> 
> The developers and contributors of AJProxy assume no liability and are not responsible for any misuse, damage, unauthorized interception, or illegal activity conducted with this software. Users bear full responsibility for compliance with all applicable local, national, and international cyber laws.

---

## 📋 Prerequisites

To build and run AJProxy, ensure you have Rust (`cargo`) installed.

### Linux (Debian/Ubuntu) Dependencies
Install the required system build tools and development libraries:
```bash
sudo apt update
sudo apt install -y build-essential pkg-config libssl-dev libgtk-3-dev
```

---

## 🚀 How to Run

Clone or navigate to the project directory and run the application using Cargo:

```bash
cargo run --release
```

---

## 🤝 Contributing

We welcome contributions from the community! Please read [CONTRIBUTING.md](CONTRIBUTING.md) to learn how to propose changes, report bugs, and build the project locally.

## 📜 License

This project is licensed under the terms specified in the LICENSE file.
