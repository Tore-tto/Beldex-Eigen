# Beldex Atomic Swap GUI

![Beldex Logo](src-gui/public/assets/logo_horizontal.png)

A cross-platform graphical user interface for performing **Beldex (BDX) to Bitcoin (BTC) Atomic Swaps**. This project is a rebranded and stabilized fork of the UnstoppableSwap GUI, tailored specifically for the Beldex ecosystem.

## 🚀 Key Features

- **Decentralized Swaps**: Swap BDX for BTC without intermediate parties.
- **Daemon Control**: Start and stop the underlying swap daemon directly from the GUI.
- **Real-time Monitoring**: Follow the swap progress with humanized, real-time logs.
- **Asset Integration**: Fully branded with official Beldex assets and 12-decimal precision (piconeros).
- **Portable Linux Builds**: Specialized Docker-based build system to ensure compatibility across different Linux distributions.

---

## 🛠️ Prerequisites

Before you begin, ensure you have the following installed:

- **Rust**: [Install Rust](https://rustup.rs/) (latest stable version).
- **Node.js & Yarn**: [Install Node.js](https://nodejs.org/) (v18+) and [Yarn](https://yarnpkg.com/).
- **Docker**: Required for running the Beldex node and for portable builds.
- **System Dependencies** (Linux only):
  ```bash
  sudo apt install -y libwebkit2gtk-4.0-dev build-essential curl wget libssl-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev
  ```

---

## 💻 Development

1.  **Clone the Repository**:
    ```bash
    git clone <repository-url>
    cd <repository-folder>
    ```

2.  **Install Frontend Dependencies**:
    ```bash
    cd src-gui
    yarn install
    cd ..
    ```

3.  **Run in Development Mode**:
    ```bash
    cargo tauri dev
    ```
    This will start the Vite frontend and the Tauri backend with hot-reloading enabled.

---

## 📦 Building for Production

### **Standard Build**
To build the application for your current operating system:
```bash
cargo tauri build
```
The binaries will be located in `src-tauri/target/release/bundle/`.

### **Portable Linux Build (Recommended for Distribution)**
To ensure your Linux binary works on older systems (resolving "GLIBC version not found" errors), use the provided Docker build environment:

1.  **Build the Docker Image**:
    ```bash
    docker build -t beldex-gui-builder -f Dockerfile.gui .
    ```
2.  **Run the Build**:
    ```bash
    docker run --rm -v $(pwd):/app beldex-gui-builder cargo tauri build
    ```
3.  The compatible **AppImage** will be generated in `target/release/bundle/appimage/`.

---

## ⚙️ Configuration & Daemons

### **Beldex Node (Testnet)**
The swap daemon requires a running Beldex node. You can run one easily using Docker:
```bash
sudo docker run -d --name beldex-testnet -p 29091:29091 --entrypoint beldexd 70d12a20f74f --testnet --rpc-bind-ip=0.0.0.0 --rpc-bind-port=29091 --confirm-external-bind --non-interactive
```

### **Swap Limits**
You can adjust the minimum and maximum swap amounts in the `config.toml` file:
- **Path**: `~/.config/bdx-btc-swap/asb/testnet/config.toml`
- **Section**: `[maker]`

---

## 🛡️ Security
This software uses cryptographic protocols that have not been formally audited. Use at your own risk. Always test with small amounts on testnet first.

## 📄 License
This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
