# Optimization and Implementation Plan

## Current Branches

### `feature/multi-video-zero-copy` (Recommended for Use)
**Status:** ✅ Optimized, ready for use
**Features:**
- 6 independent video streams in 3x2 grid layout
- 15fps frame rate limit to reduce CPU usage
- Per-camera settings (RTSP URL, codec selection)
- Global and per-stream connect/disconnect controls
- English UI for better compatibility
- Uses GStreamer with FFmpeg-based decoding

**Performance:**
- CPU usage significantly reduced compared to full frame rate
- Suitable for remote driving use cases

### `feature/zero-copy-opengl` (Research Branch)
**Status:** 📚 Research complete, implementation not started
**Features:**
- Research documentation on OpenGL context sharing
- Framework code for zero-copy implementation
- All API research completed

### `feature/zero-copy-implementation` (Work in Progress)
**Status:** 🚧 Just started
**Features:**
- Basic OpenGL context access from eframe
- Placeholder fields for GStreamer GL integration

---

## Current Optimization: 15fps Frame Limit

### How it works:
1. Added `videorate` element in GStreamer pipeline
2. Caps filter limits to 15fps
3. Reduces CPU usage by processing fewer frames

### To adjust frame rate:
Modify `src/main.rs` line ~104:
```rust
.field("framerate", gstreamer::Fraction::new(15, 1))  // Change 15 to 25 or 30
```

---

## Zero-Copy OpenGL Implementation Plan

### Phase 1: Research (✅ Complete)
- ✅ eframe OpenGL context access: `CreationContext.gl` and `Frame.gl()`
- ✅ eframe texture registration: `Frame.register_native_glow_texture()`
- ✅ GStreamer GL context wrapping: `GLContext::new_wrapped()`
- ✅ GStreamer GL texture ID extraction: `GLMemory.texture_id()`

### Phase 2: Basic Setup (🚧 In Progress)
- [x] Add gstreamer-gl dependency
- [x] Add OpenGL context fields to RemoteDriveApp
- [x] Get glow::Context from eframe::CreationContext
- [ ] Create GStreamer GLDisplay
- [ ] Wrap eframe's OpenGL context with GStreamer GLContext

### Phase 3: Pipeline Modification (Not Started)
- [ ] Add `glupload` element to pipeline
- [ ] Add `glcolorscale` element to pipeline
- [ ] Modify appsink to work with GLMemory

### Phase 4: Texture Sharing (Not Started)
- [ ] Extract texture ID from GstGLMemory
- [ ] Create glow::Texture from texture ID
- [ ] Register texture with eframe using `register_native_glow_texture()`

### Phase 5: Integration & Testing (Not Started)
- [ ] Thread synchronization for OpenGL context
- [ ] Error handling and fallback to CPU path
- [ ] Performance testing and comparison

---

## Recommendations for Remote Driving

### Short Term (Now)
1. **Use `feature/multi-video-zero-copy` branch**
2. **Adjust frame rate as needed** (15-30fps)
3. **Test in real-world conditions**

### Long Term (Future)
1. **Evaluate if zero-copy is needed** based on real-world testing
2. **Consider mature alternatives** if needed:
   - FFmpeg + OpenGL (proven by your other team)
   - Moonlight/Sunshine (game streaming tech)
3. **Continue zero-copy research** as a side project

---

## References

### eframe/egui
- `CreationContext.gl` - OpenGL context access
- `Frame.gl()` - OpenGL context access during update
- `Frame.register_native_glow_texture()` - Register native OpenGL textures

### GStreamer GL
- `GLDisplay` - OpenGL display wrapper
- `GLContext::new_wrapped()` - Wrap existing OpenGL context
- `GLMemory` - OpenGL memory buffer
- `GLMemory.texture_id()` - Get OpenGL texture ID
