use std::sync::{Arc, Mutex};

use eframe::egui;
use egui::ColorImage;
use gstreamer::prelude::*;
use gstreamer::StateChangeError;
use gstreamer_app::AppSink;

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

struct RemoteDriveApp {
    pipeline: Option<gstreamer::Pipeline>,
    video_texture: Option<egui::TextureHandle>,
    latest_frame: Arc<Mutex<Option<VideoFrame>>>,
    rtsp_url: String,
    is_playing: bool,
    codec: VideoCodec,
    speed: f32,
    battery: f32,
    gps_lat: f64,
    gps_lon: f64,
}

impl RemoteDriveApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        gstreamer::init().expect("Failed to initialize GStreamer");

        let latest_frame = Arc::new(Mutex::new(None));

        Self {
            pipeline: None,
            video_texture: None,
            latest_frame,
            rtsp_url: "rtsp://121.204.173.162:30554/rtp/TESTRTSP_1".to_string(),
            is_playing: false,
            codec: VideoCodec::H265,
            speed: 0.0,
            battery: 85.0,
            gps_lat: 39.9042,
            gps_lon: 116.4074,
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
            .max_buffers(1)
            .sync(false)
            .wait_on_eos(false)
            .build();

        pipeline.add_many([&rtspsrc, &depay, &parse, &dec, &videoconvert, &capsfilter, appsink.upcast_ref()])
            .map_err(|e| Error::GStreamer(e.to_string()))?;

        gstreamer::Element::link_many([&depay, &parse, &dec, &videoconvert, &capsfilter, appsink.upcast_ref()])
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

impl Drop for RemoteDriveApp {
    fn drop(&mut self) {
        self.stop_pipeline();
    }
}

impl eframe::App for RemoteDriveApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let frame = {
            let mut latest = self.latest_frame.lock().unwrap();
            latest.take()
        };

        if let Some(frame) = frame {
            let size = [frame.width as usize, frame.height as usize];
            let color_image = ColorImage::from_rgba_unmultiplied(size, &frame.data);

            let texture = self.video_texture.get_or_insert_with(|| {
                ctx.load_texture("video", color_image.clone(), egui::TextureOptions::default())
            });

            texture.set(color_image, egui::TextureOptions::default());
        }

        egui::SidePanel::right("control_panel")
            .min_width(280.0)
            .show(ctx, |ui| {
                ui.heading("远程驾驶控制台");
                ui.separator();

                ui.group(|ui| {
                    ui.heading("连接");
                    ui.label("RTSP URL:");
                    ui.text_edit_singleline(&mut self.rtsp_url);

                    ui.horizontal(|ui| {
                        ui.label("编码:");
                        ui.radio_value(&mut self.codec, VideoCodec::H264, "H.264");
                        ui.radio_value(&mut self.codec, VideoCodec::H265, "H.265");
                    });

                    ui.horizontal(|ui| {
                        if !self.is_playing {
                            if ui.button("▶ 连接").clicked() {
                                if let Err(e) = self.start_pipeline(ctx.clone()) {
                                    eprintln!("Failed to start pipeline: {}", e);
                                }
                            }
                        } else {
                            if ui.button("⏹ 断开").clicked() {
                                self.stop_pipeline();
                            }
                        }
                        ui.label(if self.is_playing { "🟢 连接中" } else { "⚫ 未连接" });
                    });
                });

                ui.separator();

                ui.group(|ui| {
                    ui.heading("车辆状态");
                    ui.horizontal(|ui| {
                        ui.label("速度:");
                        ui.label(format!("{:.1} km/h", self.speed));
                    });
                    ui.add(egui::Slider::new(&mut self.speed, 0.0..=120.0).text("速度"));

                    ui.horizontal(|ui| {
                        ui.label("电量:");
                        ui.label(format!("{:.1} %", self.battery));
                    });
                    ui.add(egui::ProgressBar::new(self.battery / 100.0).text(format!("{}%", self.battery)));
                });

                ui.separator();

                ui.group(|ui| {
                    ui.heading("GPS");
                    ui.horizontal(|ui| {
                        ui.label("纬度:");
                        ui.label(format!("{:.6}", self.gps_lat));
                    });
                    ui.horizontal(|ui| {
                        ui.label("经度:");
                        ui.label(format!("{:.6}", self.gps_lon));
                    });
                });

                ui.separator();

                ui.group(|ui| {
                    ui.heading("控制");
                    let (w, h) = (100.0, 80.0);
                    ui.vertical_centered(|ui| {
                        ui.add_sized([w, h], egui::Button::new("↑"));
                        ui.horizontal(|ui| {
                            ui.add_sized([w, h], egui::Button::new("←"));
                            ui.add_sized([w, h], egui::Button::new("■"));
                            ui.add_sized([w, h], egui::Button::new("→"));
                        });
                        ui.add_sized([w, h], egui::Button::new("↓"));
                    });
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("视频流");
            ui.separator();

            if let Some(texture) = &self.video_texture {
                let available_size = ui.available_size();
                let texture_size = texture.size_vec2();
                let scale = (available_size.x / texture_size.x).min(available_size.y / texture_size.y);
                let display_size = texture_size * scale;

                let (rect, _) = ui.allocate_exact_size(display_size, egui::Sense::hover());
                ui.painter().image(
                    texture.id(),
                    rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );

                let painter = ui.painter_at(rect);
                painter.rect_filled(
                    egui::Rect::from_min_size(rect.left_top() + egui::vec2(10.0, 10.0), egui::vec2(200.0, 80.0)),
                    5.0,
                    egui::Color32::from_black_alpha(180),
                );
                painter.text(
                    rect.left_top() + egui::vec2(20.0, 30.0),
                    egui::Align2::LEFT_TOP,
                    format!("速度: {:.1} km/h", self.speed),
                    egui::FontId::proportional(16.0),
                    egui::Color32::WHITE,
                );
                painter.text(
                    rect.left_top() + egui::vec2(20.0, 60.0),
                    egui::Align2::LEFT_TOP,
                    format!("电量: {:.1} %", self.battery),
                    egui::FontId::proportional(16.0),
                    egui::Color32::WHITE,
                );
                painter.circle_filled(
                    rect.center_top() + egui::vec2(0.0, 30.0),
                    25.0,
                    egui::Color32::from_rgb(if self.is_playing { 0 } else { 200 }, 0, 0),
                );
                painter.circle_filled(
                    rect.center_top() + egui::vec2(0.0, 30.0),
                    20.0,
                    egui::Color32::from_rgb(if self.is_playing { 200 } else { 0 }, 0, 0),
                );
            } else {
                ui.centered_and_justified(|ui| {
                    ui.heading("等待视频流...");
                });
            }
        });
    }
}

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("远程驾驶客户端"),
        ..Default::default()
    };

    eframe::run_native(
        "远程驾驶客户端",
        native_options,
        Box::new(|cc| Ok(Box::new(RemoteDriveApp::new(cc)))),
    )
}
