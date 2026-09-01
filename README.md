# AJProxy

<p align="center">
  <img src="banner.png" alt="AJProxy Banner" width="100%">
</p>

AJProxy is a high-performance, lightweight HTTP/HTTPS interception proxy and penetration testing workbench written from the ground up in Rust. Designed for security researchers, ethical hackers, and software engineers, AJProxy delivers low-latency packet processing with a minimal memory footprint.

Featuring a dark-themed graphical user interface built with `egui`, AJProxy provides real-time HTTP/HTTPS traffic analysis, interactive request manipulation, an advanced RFC 6455 WebSocket suite, and a customizable Intruder engine.

---

## Table of Contents

- [Overview](#overview)
- [Core Architecture & Modules](#core-architecture--modules)
  - [HTTP/HTTPS Interception & History](#httphttps-interception--history)
  - [Intruder Engine & Fuzzing Suite](#intruder-engine--fuzzing-suite)
  - [RFC 6455 WebSocket Suite](#rfc-6455-websocket-suite)
  - [Certificate Engine & OS Integration](#certificate-engine--os-integration)
  - [Repeater, Decoder, Comparer & Sitemap](#repeater-decoder-comparer--sitemap)
- [Remote Device Interception](#remote-device-interception)
- [Platform Support Status](#platform-support-status)
- [Installation & Prerequisites](#installation--prerequisites)
- [Building & Running](#building--running)
- [Legal Disclaimer](#legal-disclaimer)
- [License](#license)

---

## Overview

AJProxy operates as a local proxy listener that intercepts HTTP and HTTPS connections between target applications and remote servers. By combining Rust's async networking model with an immediate-mode UI framework, AJProxy handles high-throughput web traffic while maintaining sub-millisecond response latency.

Key highlights include:
- Pure Rust codebase (~7,500+ LOC) with no heavyweight browser or JVM overhead.
- Native TLS interception using dynamic 365-day Root CA generation.
- Real-time parameter extraction and multi-payload fuzzing.
- Full RFC 6455 WebSocket framing, masking/unmasking, and live frame modification.

---

## Core Architecture & Modules

### HTTP/HTTPS Interception & History

- **Traffic Stream**: View captured HTTP/HTTPS requests and responses in real time with status code coloring, latency measurements, and response size details.
- **Scope & Noise Suppression**: Wildcard domain filtering (`*target*`) to focus on scope targets while hiding background noise (static assets, fonts, CDNs, analytics).
- **Rule-Based Header Injection**: Dynamically inject custom headers (e.g., `X-Forwarded-For`, auth tokens) into requests and WebSocket upgrade handshakes matching defined target host patterns.
- **Live Intercept Mode**: Pause live HTTP requests before they leave your machine, modify method, URI, headers, or body, and forward or drop packets.

### Intruder Engine & Fuzzing Suite

The Intruder engine enables automated custom attacks against web endpoints:

- **Position Marking Controls**:
  - **`Add §`**: Highlights selected text in the request template editor and wraps it in position markers (`§selection§`).
  - **`Auto §`**: Automatically detects URL query parameters (`?key=val`), Form data (`key=val`), and JSON fields (`"key": "val"`) and places position markers around values.
  - **`Clear §`**: Removes all position markers with a single click.
- **Attack Modes**:
  - **Sniper**: Targets marked positions sequentially using a single payload set.
  - **Battering Ram**: Replaces all marked positions simultaneously with the same payload item.
  - **Pitchfork**: Iterates multiple payload sets in parallel (1-to-1 matching across positions).
  - **Cluster Bomb**: Performs Cartesian product combinations across all assigned payload sets.
- **Payload & Set Management**:
  - In-memory payload set creation, editing, and deletion without forced file persistence.
  - Multi-position assignment dropdowns allowing custom binding of payload sets to specific `§ Pos N` markers.
- **Pacing & Concurrency Controls**:
  - **`Delay (s)`**: Configurable time delay between request batches (e.g., `0.5`, `2.0`, or `0` for maximum speed).
  - **`Concurrency`**: Configurable batch size / concurrent threads per step.
  - **Real-Time Asynchronous Execution**: Streamed execution engine that yields live results in real-time as batches complete.
- **Filtering & Search**: Instant result filtering by status code inclusion/exclusion (e.g., `200,302` or `!404`), payload string search, and latency/size details.

### RFC 6455 WebSocket Suite

- **WS Traffic History**: Directional frame table (`⬆ Client` / `⬇ Server`) displaying Opcode types (`TEXT`, `BINARY`, `PING`, `PONG`, `CLOSE`), payload size, and timestamps.
- **Multi-Format Inspector**: View raw WebSocket payload data as formatted text, pretty JSON, or a hexadecimal dump.
- **WS Intercept Workbench**: Pause incoming or outgoing WebSocket frames on the fly, edit payload contents or Opcode types, and issue `Forward` or `Drop`.
- **WS Repeater Workbench**: Multi-tabbed interactive testing tool to construct and transmit custom WebSocket frames over `ws://` and `wss://` connections.

### Certificate Engine & OS Integration

- **Automatic CA Generation**: Generates 365-day Root CA certificates (`ajproxy_ca.crt`) stored in `~/.ajproxy/`.
- **System Trust Automation**: OS-specific installer module for Linux (`Linux CA Trust`) to install the CA certificate into local system and browser trust stores.
- **Pass-Through Engine**: Bypass TLS decryption for non-target streaming endpoints (e.g., YouTube video CDNs) to preserve bandwidth and performance.

### Repeater, Decoder, Comparer & Sitemap

- **HTTP Repeater**: Modify and re-send captured HTTP requests in isolated tabbed environments.
- **Decoder**: Perform URL, Base64, Hex, and HTML entity encoding/decoding operations.
- **Comparer**: Visual word and byte-level diff utility to compare HTTP responses.
- **Sitemap**: Auto-generated hierarchical site tree displaying discovered hosts, endpoints, and paths.

---

## Remote Device Interception

To intercept HTTP/HTTPS traffic from mobile devices, tablets, or secondary machines on your local network:

1. Open **Settings** in AJProxy.
2. Set the listener **Bind Address** to `0.0.0.0` and confirm the active port (default `8080`).
3. Configure the remote device's proxy settings to point to your host IP: `http://<YOUR_IP>:8080`.
4. Open a browser on the remote device and visit `http://<YOUR_IP>:8080/cert` to download and install the Root CA Certificate.

---

## Platform Support Status

| Platform | Support Status | Root CA Automation | Notes |
| :--- | :--- | :--- | :--- |
| **Linux** | Supported | Fully Automated | Tested on Debian/Ubuntu/Kali. Recommended for daily security research. |
| **Windows** | In Progress | Manual / Experimental | Core proxy works. System trust integration undergoing testing. |
| **macOS** | In Progress | Manual / Experimental | Core proxy works. System trust integration undergoing testing. |

---

## Installation & Prerequisites

Ensure Rust and `cargo` are installed on your system.

### Linux System Dependencies

On Debian, Ubuntu, or Kali Linux, install the required build packages:

```bash
sudo apt update
sudo apt install -y build-essential pkg-config libssl-dev libgtk-3-dev
```

---

## Building & Running

Clone the repository and compile in release mode for optimal performance:

```bash
git clone https://github.com/jafarov007/AJProxy.git
cd AJProxy
cargo run --release
```

To run a quick debug check:

```bash
cargo check
```

---

## Legal Disclaimer

> [!IMPORTANT]
> **Authorized Testing Only**: AJProxy is designed exclusively for authorized penetration testing, security auditing, and educational research. You must obtain explicit written permission from the system owner before analyzing any network, application, or system.
> 
> The authors and maintainers of AJProxy assume no responsibility for unauthorized access, misuse, or data loss resulting from the use of this software.

---

## License

This project is licensed under the terms defined in the [LICENSE](LICENSE) file.
