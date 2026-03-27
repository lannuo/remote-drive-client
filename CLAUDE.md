# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a remote driving desktop client built in Rust for real-time video streaming and vehicle control using RTSP streams.

## Key Dependencies

- GUI: eframe + egui 0.29
- Multimedia: GStreamer 0.23 (with v1_24 features)
- Requires system GStreamer libraries (see setup)

## Setup & Dependencies

**Ubuntu/Debian system libraries:**
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

## Common Commands

```bash
# Build and run in development mode
cargo run

# Build and run in release mode (recommended for performance)
cargo run --release

# Check compilation
cargo check

# Run tests
cargo test

# Format code
cargo fmt

# Clippy lint check
cargo clippy
```

## Architecture

### Current Architecture (Single Video)

The application uses a GStreamer pipeline that:
1. Receives RTSP stream via `rtspsrc`
2. Depayloads and parses H.264/H.265 video
3. Decodes using libav
4. Converts to RGBA format
5. Pulls frames to CPU memory via `appsink`
6. Copies to egui textures for display

**Key components in `src/main.rs`:**
- `RemoteDriveApp` - Core application state with GStreamer pipeline management
- `VideoFrame` - Thread-safe frame buffer using `Arc<Mutex<Option<VideoFrame>>>`
- GStreamer pipeline with dynamic pad handling for RTSP
- egui UI with video display, HUD overlay, and control panel

### Future Architecture (In Development)

The `feature/multi-video-zero-copy` branch aims to implement:
- 6 independent GStreamer pipelines
- Zero-copy OpenGL texture sharing using `glupload` + `glcolorscale`
- Direct texture reference in egui for GPU-only rendering

## Project Structure

```
remote-drive-client/
├── Cargo.toml          (Rust package manifest)
├── README.md           (Chinese documentation)
└── src/
    └── main.rs         (Single-file application - 366 lines)
```

## Important Notes

- The entire application is contained in `src/main.rs`
- Supports both H.264 and H.265 codecs
- Optimized for low-latency with `latency=0` and `drop-on-latency=true`
- Uses ZLMediaKit as the recommended RTSP server
