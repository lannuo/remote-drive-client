# Zero-Copy OpenGL Implementation Plan

## Overview
This document outlines the plan for implementing zero-copy OpenGL texture sharing between GStreamer and egui.

## Architecture

### Current State
- 6 independent GStreamer pipelines using appsink
- CPU memory copy between GStreamer and egui
- High CPU usage for 6 simultaneous streams

### Target State
- 6 independent GStreamer pipelines using OpenGL
- Zero-copy texture sharing
- Minimal CPU usage

## Implementation Steps

### Step 1: Explore eframe's OpenGL Context Access
- Understand how to get the OpenGL context from eframe
- Check if we can use glow or wgpu to access the context
- Look at `eframe::Frame` methods: `get_mut_render_state()`, `wgpu_render_state()`, etc.

### Step 2: Set up GStreamer GL Context
- Create a GstGLContext
- Share the context with eframe's OpenGL context
- Use `gst_gl_context_new_wrapped()` or similar

### Step 3: Modify GStreamer Pipeline
- Replace appsink with:
  - `glupload` - Upload video to OpenGL texture
  - `glcolorscale` - Convert color space in GPU
  - `glsinkbin` or custom app sink that gets texture IDs

### Step 4: Extract Texture IDs from GStreamer
- Get OpenGL texture IDs from GstGLMemory
- Store texture IDs for each video stream
- Synchronize texture access between threads

### Step 5: Render Textures in egui
- Create custom render pass for egui
- Use GStreamer's texture IDs directly
- Avoid CPU-GPU memory copies

## Challenges
1. OpenGL context sharing between GStreamer and eframe
2. Thread synchronization for texture access
3. egui's render pipeline integration
4. Platform-specific OpenGL setup (X11, Wayland, Windows, macOS)

## References
- GStreamer GL documentation
- eframe/egui rendering documentation
- OpenGL context sharing best practices
