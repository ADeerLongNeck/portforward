# Port Forward

一个轻量、快速、安全的跨平台的网络工具，支持 GUI 和 CLI 两种模式。

## 功能特性

- 🚀 **高性能**: 基于 Rust 和 Tokio 异步运行时
- 🔐 **安全**: 支持密码认证和 AES-GCM 加密
- 🌐 **SOCKS5 代理**: 服务端支持 SOCKS5 协议
- 📊 **实时统计**: 显示连接数、流量、速率等
- 💻 **双模式**: 支持 GUI 和命令行两种运行方式
- 🔄 **跨平台**: 支持 Windows、Linux、macOS

## 构建说明

### 环境要求

- Rust 1.70+
- Node.js 18+ (仅 GUI 模式需要)
- pnpm 或 npm (仅 GUI 模式需要)

### 构建命令

#### GUI 模式 (Windows/macOS/Linux 桌面应用)

```bash
# 安装前端依赖
npm install

# 开发模式运行
npm run tauri dev

# 构建发布版本
npm run tauri build
```

构建产物位置：
- Windows: `src-tauri/target/release/bundle/msi/`
- macOS: `src-tauri/target/release/bundle/dmg/`
- Linux: `src-tauri/target/release/bundle/deb/` 或 `appimage/`

#### CLI 模式 (Linux 服务器/命令行)

```bash
cd src-tauri

# 构建发布版本
cargo build --features cli --release
```

构建产物位置：`src-tauri/target/release/port-forward`

## 运行说明

### GUI 模式

1. 启动应用程序
2. 选择运行模式：
   - **客户端模式**: 连接到远程服务端
   - **服务端模式**: 监听端口等待客户端连接

#### 服务端配置

| 配置项 | 说明 |
|--------|------|
| 监听端口 | 隧道服务监听端口 (默认 5173) |
| 连接密码 | 客户端连接密码 |
| 本地转发端口 | SOCKS5 代理监听端口 (如 1080, 3389) |

#### 客户端配置

| 配置项 | 说明 |
|--------|------|
| 服务端地址 | 远程服务端 IP 或域名 |
| 服务端端口 | 远程服务端端口 |
| 连接密码 | 与服务端一致的密码 |

### CLI 模式

#### 服务端模式

```bash
# 基本用法
./port-forward server --port 5173 --password yourpassword

# 指定转发端口
./port-forward server --port 5173 --password yourpassword --forward "1080,3389,3306"

# 查看帮助
./port-forward server --help
```

参数说明：
- `-p, --port`: 隧道服务监听端口 (默认 5173)
- `-P, --password`: 连接密码 (必填)
- `-f, --forward`: 本地转发端口，逗号分隔 (可选)

#### 客户端模式

```bash
# 基本用法
./port-forward client --host server.example.com --port 5173 --password yourpassword

# 查看帮助
./port-forward client --help
```

参数说明：
- `-H, --host`: 服务端地址 (必填)
- `-p, --port`: 服务端端口 (默认 5173)
- `-P, --password`: 连接密码 (必填)

### 使用示例

#### 场景：通过远程网络访问内网服务

1. **在内网机器上运行客户端**:
   ```bash
   ./port-forward client --host your-server.com --port 5173 --password secret123
   ```

2. **在公网服务器上运行服务端**:
   ```bash
   ./port-forward server --port 5173 --password secret123 --forward "1080"
   ```

3. **配置代理**:
   - 将浏览器或其他应用的代理设置为 `服务器IP:1080` (SOCKS5)
   - 所有流量将通过隧道转发到内网客户端，再由客户端访问目标

### 统计数据说明

| 指标 | 说明 |
|------|------|
| 已连接客户端 | 当前活跃的连接数 (隧道客户端 + 转发连接) |
| 上传速率 | 从本地发送到隧道的速率 |
| 下载速率 | 从隧道接收到本地的速率 |
| 总流量 | 累计传输的字节数 |

## 项目结构

```
port-forward-tauri/
├── src/                    # Vue 前端代码 (GUI)
│   ├── App.vue            # 主应用组件
│   ├── stores/            # Pinia 状态管理
│   └── types/             # TypeScript 类型定义
├── src-tauri/             # Rust 后端代码
│   ├── src/
│   │   ├── commands/      # Tauri 命令
│   │   ├── config/        # 配置管理
│   │   ├── crypto/        # 加密模块
│   │   ├── forward/       # 端口转发
│   │   ├── protocol/      # 通信协议
│   │   ├── stats/         # 统计模块
│   │   ├── tunnel/        # 隧道管理
│   │   ├── cli.rs         # CLI 入口
│   │   ├── gui.rs         # GUI 入口
│   │   └── lib.rs         # 库入口
│   └── Cargo.toml         # Rust 依赖配置
├── package.json
└── README.md
```

## 技术栈

- **前端**: Vue 3 + TypeScript + Pinia
- **后端**: Rust + Tauri 2 + Tokio
- **加密**: AES-256-GCM
- **协议**: 自定义帧协议 + SOCKS5

## License

MIT
