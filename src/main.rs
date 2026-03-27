use std::sync::{Arc, Mutex};

use eframe::egui;
use egui::ColorImage;
use gstreamer::prelude::*;
use gstreamer::StateChangeError;
use gstreamer_app::AppSink;
use gstreamer_gl as gst_gl;

const NUM_VIDEOS: usize = 6;

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("GStreamer error: {0}")]
    GStreamer(String),
    #[error("GStreamer state change error: {0}")]
    StateChange(#[from] StateChangeError),
}

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VideoCodec {
    H264,
    H265,
}

#[derive(Clone)]
struct VideoFrame {
    data: Arc<Vec<u8>>,
    width: u32,
    height: u32,
}

struct VideoPipeline {
    pipeline: Option<gstreamer::Pipeline>,
    latest_frame: Arc<Mutex<Option<VideoFrame>>>,
    is_playing: bool,
    rtsp_url: String,
    codec: VideoCodec,
}

impl VideoPipeline {
    fn new(default_url: String) -> Self {
        Self {
            pipeline: None,
            latest_frame: Arc::new(Mutex::new(None)),
            is_playing: false,
            rtsp_url: default_url,
            codec: VideoCodec::H265,
        }
    }

    fn start_pipeline(&mut self, egui_ctx: egui::Context) -> Result<()> {
        if self.pipeline.is_some() {
            self.stop_pipeline();
        }

        let pipeline = gstreamer::Pipeline::new();

        let rtspsrc = gstreamer::ElementFactory::make("rtspsrc")
            .name("rtspsrc")
            .property("location", &self.rtsp_url)
            .property("latency", 0u32)
            .property("drop-on-latency", true)
            .build()
            .map_err(|e| Error::GStreamer(e.to_string()))?;

        let (depay_name, parse_name, dec_name) = match self.codec {
            VideoCodec::H264 => ("rtph264depay", "h264parse", "avdec_h264"),
            VideoCodec::H265 => ("rtph265depay", "h265parse", "avdec_h265"),
        };

        let depay = gstreamer::ElementFactory::make(depay_name)
            .name("depay")
            .build()
            .map_err(|e| Error::GStreamer(e.to_string()))?;

        let parse = gstreamer::ElementFactory::make(parse_name)
            .name("parse")
            .property("config-interval", -1i32)
            .build()
            .map_err(|e| Error::GStreamer(e.to_string()))?;

        let dec = gstreamer::ElementFactory::make(dec_name)
            .name("dec")
            .build()
            .map_err(|e| Error::GStreamer(e.to_string()))?;

        let videoconvert = gstreamer::ElementFactory::make("videoconvert")
            .name("videoconvert")
            .build()
            .map_err(|e| Error::GStreamer(e.to_string()))?;

        let videorate = gstreamer::ElementFactory::make("videorate")
            .name("videorate")
            .build()
            .map_err(|e| Error::GStreamer(e.to_string()))?;

        let rate_capsfilter = gstreamer::ElementFactory::make("capsfilter")
            .name("rate_capsfilter")
            .property(
                "caps",
                &gstreamer::Caps::builder("video/x-raw")
                    .field("framerate", gstreamer::Fraction::new(15, 1))
                    .build(),
            )
            .build()
            .map_err(|e| Error::GStreamer(e.to_string()))?;

        let capsfilter = gstreamer::ElementFactory::make("capsfilter")
            .name("capsfilter")
            .property(
                "caps",
                &gstreamer::Caps::builder("video/x-raw")
                    .field("format", "RGBA")
                    .build(),
            )
            .build()
            .map_err(|e| Error::GStreamer(e.to_string()))?;

        let appsink = AppSink::builder()
            .name("appsink")
            .caps(&gstreamer::Caps::builder("video/x-raw")
                .field("format", "RGBA")
                .build())
            .drop(true)
            .max_buffers(2)
            .buffer_list(false)
            .sync(false)
            .wait_on_eos(false)
            .build();

        pipeline.add_many([&rtspsrc, &depay, &parse, &dec, &videoconvert, &videorate, &rate_capsfilter, &capsfilter, appsink.upcast_ref()])
            .map_err(|e| Error::GStreamer(e.to_string()))?;

        gstreamer::Element::link_many([&depay, &parse, &dec, &videoconvert, &videorate, &rate_capsfilter, &capsfilter, appsink.upcast_ref()])
            .map_err(|e| Error::GStreamer(e.to_string()))?;

        let pipeline_weak = pipeline.downgrade();
        rtspsrc.connect_pad_added(move |_rtspsrc, pad| {
            let Some(pipeline) = pipeline_weak.upgrade() else { return };
            let Some(depay) = pipeline.by_name("depay") else { return };
            let Some(sink_pad) = depay.static_pad("sink") else { return };

            if sink_pad.is_linked() {
                return;
            }

            if pad.name().starts_with("recv_rtp_src_") {
                let _ = pad.link(&sink_pad);
            }
        });

        let latest_frame = self.latest_frame.clone();

        appsink.set_callbacks(
            gstreamer_app::AppSinkCallbacks::builder()
                .new_sample(move |appsink| {
                    let sample = appsink.pull_sample().map_err(|_| gstreamer::FlowError::Eos)?;
                    let buffer = sample.buffer().ok_or(gstreamer::FlowError::Error)?;
                    let caps = sample.caps().ok_or(gstreamer::FlowError::Error)?;
                    let s = caps.structure(0).ok_or(gstreamer::FlowError::Error)?;

                    let width: i32 = s.get("width").map_err(|_| gstreamer::FlowError::Error)?;
                    let height: i32 = s.get("height").map_err(|_| gstreamer::FlowError::Error)?;

                    let map = buffer.map_readable().map_err(|_| gstreamer::FlowError::Error)?;
                    let data = Arc::new(map.to_vec());

                    let mut latest = latest_frame.lock().unwrap();
                    *latest = Some(VideoFrame { data, width: width as u32, height: height as u32 });
                    egui_ctx.request_repaint();

                    Ok(gstreamer::FlowSuccess::Ok)
                })
                .build(),
        );

        pipeline.set_state(gstreamer::State::Playing)?;
        self.pipeline = Some(pipeline);
        self.is_playing = true;

        Ok(())
    }

    fn stop_pipeline(&mut self) {
        if let Some(pipeline) = self.pipeline.take() {
            let _ = pipeline.set_state(gstreamer::State::Null);
        }
        self.is_playing = false;
    }
}

impl Drop for VideoPipeline {
    fn drop(&mut self) {
        self.stop_pipeline();
    }
}

struct RemoteDriveApp {
    pipelines: [VideoPipeline; NUM_VIDEOS],
    video_textures: [Option<egui::TextureHandle>; NUM_VIDEOS],
    speed: f32,
    battery: f32,
    gps_lat: f64,
    gps_lon: f64,
    global_connected: bool,
    // Zero-copy OpenGL fields (work in progress)
    gl_context: Option<Arc<eframe::glow::Context>>,
    gst_gl_display: Option<gst_gl::GLDisplay>,
    gst_gl_context: Option<gst_gl::GLContext>,
}

impl RemoteDriveApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        gstreamer::init().expect("Failed to initialize GStreamer");

        // Get OpenGL context from eframe (for zero-copy, work in progress)
        let gl_context = cc.gl.clone();

        let default_urls = [
            "rtsp://121.204.173.162:30554/rtp/TESTRTSP_1".to_string(),
            "rtsp://121.204.173.162:30554/rtp/TESTRTSP_2".to_string(),
            "rtsp://121.204.173.162:30554/rtp/TESTRTSP_3".to_string(),
            "rtsp://121.204.173.162:30554/rtp/TESTRTSP_4".to_string(),
            "rtsp://121.204.173.162:30554/rtp/TESTRTSP_5".to_string(),
            "rtsp://121.204.173.162:30554/rtp/TESTRTSP_6".to_string(),
        ];

        let pipelines = [
            VideoPipeline::new(default_urls[0].clone()),
            VideoPipeline::new(default_urls[1].clone()),
            VideoPipeline::new(default_urls[2].clone()),
            VideoPipeline::new(default_urls[3].clone()),
            VideoPipeline::new(default_urls[4].clone()),
            VideoPipeline::new(default_urls[5].clone()),
        ];

        Self {
            pipelines,
            video_textures: Default::default(),
            speed: 0.0,
            battery: 85.0,
            gps_lat: 39.9042,
            gps_lon: 116.4074,
            global_connected: false,
            gl_context,
            gst_gl_display: None,
            gst_gl_context: None,
        }
    }

    fn start_all_pipelines(&mut self, ctx: egui::Context) {
        for i in 0..NUM_VIDEOS {
            if let Err(e) = self.pipelines[i].start_pipeline(ctx.clone()) {
                eprintln!("Failed to start pipeline {}: {}", i, e);
            }
        }
        self.global_connected = true;
    }

    fn stop_all_pipelines(&mut self) {
        for i in 0..NUM_VIDEOS {
            self.pipelines[i].stop_pipeline();
        }
        self.global_connected = false;
    }
}

impl Drop for RemoteDriveApp {
    fn drop(&mut self) {
        self.stop_all_pipelines();
    }
}

impl eframe::App for RemoteDriveApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        for i in 0..NUM_VIDEOS {
            let frame = {
                let mut latest = self.pipelines[i].latest_frame.lock().unwrap();
                latest.take()
            };

            if let Some(frame) = frame {
                let size = [frame.width as usize, frame.height as usize];
                let color_image = ColorImage::from_rgba_unmultiplied(size, &frame.data);

                let texture = self.video_textures[i].get_or_insert_with(|| {
                    ctx.load_texture(format!("video_{}", i), color_image.clone(), egui::TextureOptions::default())
                });

                texture.set(color_image, egui::TextureOptions::default());
            }
        }

        egui::SidePanel::right("control_panel")
            .min_width(280.0)
            .show(ctx, |ui| {
                ui.heading("Control Panel");
                ui.separator();

                ui.group(|ui| {
                    ui.heading("Global Control");
                    ui.horizontal(|ui| {
                        if !self.global_connected {
                            if ui.button("[Connect] All").clicked() {
                                self.start_all_pipelines(ctx.clone());
                            }
                        } else {
                            if ui.button("[Disconnect] All").clicked() {
                                self.stop_all_pipelines();
                            }
                        }
                        ui.label(if self.global_connected { "[Connected]" } else { "[Disconnected]" });
                    });
                });

                ui.separator();

                ui.group(|ui| {
                    ui.heading("Camera Settings");
                    for i in 0..NUM_VIDEOS {
                        ui.collapsing(format!("Camera {}", i + 1), |ui| {
                            ui.label("RTSP URL:");
                            ui.text_edit_singleline(&mut self.pipelines[i].rtsp_url);

                            ui.horizontal(|ui| {
                                ui.label("Codec:");
                                ui.radio_value(&mut self.pipelines[i].codec, VideoCodec::H264, "H.264");
                                ui.radio_value(&mut self.pipelines[i].codec, VideoCodec::H265, "H.265");
                            });
                        });
                    }
                });

                ui.separator();

                ui.group(|ui| {
                    ui.heading("Vehicle Status");
                    ui.horizontal(|ui| {
                        ui.label("Speed:");
                        ui.label(format!("{:.1} km/h", self.speed));
                    });
                    ui.add(egui::Slider::new(&mut self.speed, 0.0..=120.0).text("Speed"));

                    ui.horizontal(|ui| {
                        ui.label("Battery:");
                        ui.label(format!("{:.1} %", self.battery));
                    });
                    ui.add(egui::ProgressBar::new(self.battery / 100.0).text(format!("{}%", self.battery)));
                });

                ui.separator();

                ui.group(|ui| {
                    ui.heading("GPS");
                    ui.horizontal(|ui| {
                        ui.label("Lat:");
                        ui.label(format!("{:.6}", self.gps_lat));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Lon:");
                        ui.label(format!("{:.6}", self.gps_lon));
                    });
                });

                ui.separator();

                ui.group(|ui| {
                    ui.heading("Controls");
                    let (w, h) = (100.0, 80.0);
                    ui.vertical_centered(|ui| {
                        ui.add_sized([w, h], egui::Button::new("Forward"));
                        ui.horizontal(|ui| {
                            ui.add_sized([w, h], egui::Button::new("Left"));
                            ui.add_sized([w, h], egui::Button::new("Stop"));
                            ui.add_sized([w, h], egui::Button::new("Right"));
                        });
                        ui.add_sized([w, h], egui::Button::new("Back"));
                    });
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Multi-Video Streams (6 channels)");
            ui.separator();

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    egui::Grid::new("video_grid")
                        .num_columns(2)
                        .spacing([10.0, 10.0])
                        .show(ui, |ui| {
                            for row in 0..3 {
                                for col in 0..2 {
                                    let i = row * 2 + col;
                                    ui.group(|ui| {
                                        ui.set_min_width(300.0);

                                        ui.horizontal(|ui| {
                                            ui.heading(format!("Camera {}", i + 1));
                                            ui.label(if self.pipelines[i].is_playing { "[ON]" } else { "[OFF]" });
                                        });

                                        ui.horizontal(|ui| {
                                            if !self.pipelines[i].is_playing {
                                                if ui.button("Connect").clicked() {
                                                    if let Err(e) = self.pipelines[i].start_pipeline(ctx.clone()) {
                                                        eprintln!("Failed to start pipeline {}: {}", i, e);
                                                    }
                                                }
                                            } else {
                                                if ui.button("Disconnect").clicked() {
                                                    self.pipelines[i].stop_pipeline();
                                                }
                                            }
                                        });

                                        ui.separator();

                                        if let Some(texture) = &self.video_textures[i] {
                                            let texture_size = texture.size_vec2();
                                            let available_width = ui.available_width().max(280.0);
                                            let scale = available_width / texture_size.x;
                                            let display_size = texture_size * scale;

                                            let (rect, _) = ui.allocate_exact_size(display_size, egui::Sense::hover());
                                            ui.painter().image(
                                                texture.id(),
                                                rect,
                                                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                                egui::Color32::WHITE,
                                            );
                                        } else {
                                            ui.centered_and_justified(|ui| {
                                                ui.set_min_size(egui::vec2(280.0, 160.0));
                                                ui.label("Waiting for video...");
                                            });
                                        }
                                    });
                                }
                                ui.end_row();
                            }
                        });
                });
        });
    }
}

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1600.0, 900.0])
            .with_min_inner_size([1280.0, 720.0])
            .with_title("Remote Drive Client - Multi Video"),
        ..Default::default()
    };

    eframe::run_native(
        "Remote Drive Client - Multi Video",
        native_options,
        Box::new(|cc| Ok(Box::new(RemoteDriveApp::new(cc)))),
    )
}
