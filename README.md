# AJProxy

<p align="center">
  <img src="banner.png" alt="AJProxy Banner" width="100%">
</p>

AJProxy is an ultra-fast, lightweight, and professional HTTP/HTTPS interception proxy and security testing tool built from the ground up in Rust. Leveraging Rust's memory safety, concurrency model, and high performance, AJProxy delivers sub-millisecond packet interception with a minimal system memory footprint. 

It features a modern graphical user interface powered by egui, enabling security researchers and developers to intercept, inspect, and modify web traffic in real time.

## Key Features

- **Built with Rust**: Inherits Rust's performance, stability, safety, and speed.
- **Multi-Listener Proxying**: Configure multiple ports and binding interfaces with support for HTTP/1.1, HTTP/2, and automatic protocol detection.
- **Real-Time Interception**: Intercept and modify HTTP/HTTPS requests and responses on the fly.
- **Dynamic SSL/TLS MITM Decryption**: Automatically generates a 365-day compliant Root CA. Offers one-click automatic CA trust installation for Linux/Ubuntu systems and major browsers (Chrome/Firefox).
- **SSL Passthrough**: Exclude specific hostnames from decryption to pass traffic through seamlessly.
- **Security Testing Toolkit**: Includes tools such as Dashboard, Repeater, Sitemap, Comparer, Decoder, Traffic viewer, and Intruder/Bruteforce modules.

## Prerequisites

To build and run AJProxy, ensure you have Rust (cargo) installed.

### Linux (Debian/Ubuntu) Dependencies
Install the required system build tools and development libraries:
```bash
sudo apt update
sudo apt install -y build-essential pkg-config libssl-dev libgtk-3-dev
```

## How to Run

Clone or navigate to the project directory and run the application using Cargo:

```bash
cargo run --release
```

## Contributing

 We welcome contributions from the community! Please read [CONTRIBUTING.md](CONTRIBUTING.md) to learn how to propose changes, report bugs, and build the project locally.

## License

This project is licensed under the terms specified in the LICENSE file.
