# 远程驾驶客户端

基于 Rust + egui + GStreamer 的远程驾驶桌面客户端。

## 功能特性

- ✅ 支持 H.264/H.265 视频流
- ✅ RTSP 拉流（支持 ZLMediaKit）
- ✅ 低延迟播放优化
- ✅ egui 控制面板（车辆状态、GPS、控制按钮）
- ✅ HUD 信息叠加显示

## 技术栈

- **GUI**: eframe + egui
- **多媒体**: GStreamer (gstreamer-rs)
- **语言**: Rust

## 环境依赖

### Ubuntu/Debian

```bash
sudo apt install libgstreamer1.0-dev \
    libgstreamer-plugins-base1.0-dev \
    libgstreamer-plugins-bad1.0-dev \
    gstreamer1.0-plugins-base \
    gstreamer1.0-plugins-good \
    gstreamer1.0-plugins-bad \
    gstreamer1.0-plugins-ugly \
    gstreamer1.0-libav
```

## 编译运行

```bash
# 开发模式
cargo run

# 发布模式（推荐，更流畅）
cargo run --release
```

## 使用说明

1. 在右侧面板输入 RTSP URL
2. 选择编码格式（H.264 或 H.265）
3. 点击"连接"开始播放

## 推流测试（ZLMediaKit）

使用 FFmpeg 推流到 ZLMediaKit：

```bash
# H.264 推流
ffmpeg -re -stream_loop -1 -i test.mp4 -c:v libx264 -tune zerolatency \
    -c:a copy -rtsp_transport tcp -f rtsp rtsp://<server:port/rtp/STREAM_ID

# H.265 推流
ffmpeg -re -stream_loop -1 -i test.mp4 -c:v libx265 -tune zerolatency \
    -c:a copy -rtsp_transport tcp -f rtsp rtsp://<server:port/rtp/STREAM_ID
```

## 分支说明

- `master` - 单路视频稳定版本
- `feature/multi-video-zero-copy` - 多路视频零拷贝开发分支（开发中）

## 项目结构

```
remote-drive-client/
├── Cargo.toml
├── README.md
└── src/
    └── main.rs
```

## License

MIT
