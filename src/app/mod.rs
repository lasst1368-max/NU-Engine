use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Icon, Window, WindowAttributes, WindowId};

use crate::core::ApiError;

#[derive(Debug, Clone)]
pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "nu".to_string(),
            width: 1280,
            height: 720,
        }
    }
}

pub trait WindowApp {
    fn window_config(&self) -> WindowConfig;

    fn resumed(&mut self, _window: &Window) -> Result<(), ApiError> {
        Ok(())
    }

    fn window_event(
        &mut self,
        _window: &Window,
        _event_loop: &ActiveEventLoop,
        _event: &WindowEvent,
    ) -> Result<(), ApiError> {
        Ok(())
    }

    fn about_to_wait(&mut self, _window: &Window) -> Result<(), ApiError> {
        Ok(())
    }

    fn exiting(&mut self) {}
}

pub fn run_window_app<T>(app: T) -> Result<(), ApiError>
where
    T: WindowApp + 'static,
{
    let event_loop = EventLoop::new().map_err(|err| ApiError::Window {
        reason: format!("failed to create event loop: {err}"),
    })?;

    let mut handler = WindowAppHandler {
        app,
        window: None,
        window_id: None,
        fatal_error: None,
    };

    event_loop
        .run_app(&mut handler)
        .map_err(|err| ApiError::Window {
            reason: format!("event loop error: {err}"),
        })?;

    if let Some(error) = handler.fatal_error {
        return Err(error);
    }

    Ok(())
}

struct WindowAppHandler<T>
where
    T: WindowApp,
{
    app: T,
    window: Option<Window>,
    window_id: Option<WindowId>,
    fatal_error: Option<ApiError>,
}

impl<T> WindowAppHandler<T>
where
    T: WindowApp,
{
    fn create_window(&mut self, event_loop: &ActiveEventLoop) -> Result<(), ApiError> {
        if self.window.is_some() {
            return Ok(());
        }

        let config = self.app.window_config();
        let attributes: WindowAttributes = Window::default_attributes()
            .with_title(config.title)
            .with_inner_size(LogicalSize::new(config.width as f64, config.height as f64))
            .with_window_icon(build_nu_window_icon());
        let window = event_loop
            .create_window(attributes)
            .map_err(|err| ApiError::Window {
                reason: format!("failed to create window: {err}"),
            })?;

        self.window_id = Some(window.id());
        self.app.resumed(&window)?;
        self.window = Some(window);
        Ok(())
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: ApiError) {
        self.fatal_error = Some(error);
        event_loop.exit();
    }
}

impl<T> ApplicationHandler for WindowAppHandler<T>
where
    T: WindowApp,
{
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let Err(error) = self.create_window(event_loop) {
            self.fail(event_loop, error);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if Some(window_id) != self.window_id {
            return;
        }

        if let WindowEvent::CloseRequested = event {
            event_loop.exit();
            return;
        }

        if let Some(window) = self.window.as_ref() {
            if let Err(error) = self.app.window_event(window, event_loop, &event) {
                self.fail(event_loop, error);
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(window) = self.window.as_ref() {
            if let Err(error) = self.app.about_to_wait(window) {
                self.fail(event_loop, error);
            }
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.app.exiting();
    }
}

fn build_nu_window_icon() -> Option<Icon> {
    const SIZE: u32 = 128;
    const VIEW_W: f32 = 176.0;
    const VIEW_H: f32 = 214.0;
    const PAD: f32 = 10.0;
    let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];
    let scale = ((SIZE as f32 - PAD * 2.0) / VIEW_W).min((SIZE as f32 - PAD * 2.0) / VIEW_H);
    let offset_x = (SIZE as f32 - VIEW_W * scale) * 0.5;
    let offset_y = (SIZE as f32 - VIEW_H * scale) * 0.5;

    let map = |x: f32, y: f32| -> [f32; 2] { [offset_x + x * scale, offset_y + y * scale] };
    let top = [
        map(88.0, 0.0),
        map(176.0, 46.0),
        map(88.0, 92.0),
        map(0.0, 46.0),
    ];
    let left = [
        map(0.0, 46.0),
        map(88.0, 92.0),
        map(88.0, 210.0),
        map(0.0, 164.0),
    ];
    let right = [
        map(88.0, 92.0),
        map(176.0, 46.0),
        map(176.0, 164.0),
        map(88.0, 210.0),
    ];

    fill_polygon(&mut rgba, SIZE, &left, [136, 0, 0, 255]);
    fill_polygon(&mut rgba, SIZE, &right, [238, 32, 16, 255]);
    fill_polygon(&mut rgba, SIZE, &top, [255, 112, 85, 255]);

    let white = [255, 255, 255, 255];
    stroke_polyline(
        &mut rgba,
        SIZE,
        &[map(88.0, 0.0), map(88.0, 92.0)],
        1.6,
        white,
    );
    stroke_closed_polygon(&mut rgba, SIZE, &top, 1.8, white);
    stroke_polyline(
        &mut rgba,
        SIZE,
        &[map(0.0, 46.0), map(88.0, 210.0)],
        1.6,
        white,
    );
    stroke_closed_polygon(&mut rgba, SIZE, &left, 1.8, white);
    stroke_polyline(
        &mut rgba,
        SIZE,
        &[map(176.0, 46.0), map(88.0, 210.0)],
        1.6,
        white,
    );
    stroke_closed_polygon(&mut rgba, SIZE, &right, 1.8, white);

    Icon::from_rgba(rgba, SIZE, SIZE).ok()
}

fn fill_polygon(rgba: &mut [u8], width: u32, points: &[[f32; 2]], color: [u8; 4]) {
    if points.len() < 3 {
        return;
    }
    let height = width as i32;
    let min_x = points
        .iter()
        .map(|p| p[0])
        .fold(f32::INFINITY, f32::min)
        .floor()
        .max(0.0) as i32;
    let max_x = points
        .iter()
        .map(|p| p[0])
        .fold(f32::NEG_INFINITY, f32::max)
        .ceil()
        .min(width as f32 - 1.0) as i32;
    let min_y = points
        .iter()
        .map(|p| p[1])
        .fold(f32::INFINITY, f32::min)
        .floor()
        .max(0.0) as i32;
    let max_y = points
        .iter()
        .map(|p| p[1])
        .fold(f32::NEG_INFINITY, f32::max)
        .ceil()
        .min(height as f32 - 1.0) as i32;

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let sample = [x as f32 + 0.5, y as f32 + 0.5];
            if point_in_convex_polygon(sample, points) {
                blend_pixel(rgba, width, x as u32, y as u32, color);
            }
        }
    }
}

fn stroke_closed_polygon(
    rgba: &mut [u8],
    width: u32,
    points: &[[f32; 2]],
    thickness: f32,
    color: [u8; 4],
) {
    if points.len() < 2 {
        return;
    }
    for index in 0..points.len() {
        let start = points[index];
        let end = points[(index + 1) % points.len()];
        stroke_line(rgba, width, start, end, thickness, color);
    }
}

fn stroke_polyline(
    rgba: &mut [u8],
    width: u32,
    points: &[[f32; 2]],
    thickness: f32,
    color: [u8; 4],
) {
    if points.len() < 2 {
        return;
    }
    for pair in points.windows(2) {
        stroke_line(rgba, width, pair[0], pair[1], thickness, color);
    }
}

fn stroke_line(
    rgba: &mut [u8],
    width: u32,
    start: [f32; 2],
    end: [f32; 2],
    thickness: f32,
    color: [u8; 4],
) {
    let height = width as i32;
    let radius = thickness * 0.5;
    let min_x = (start[0].min(end[0]) - radius - 1.0).floor().max(0.0) as i32;
    let max_x = (start[0].max(end[0]) + radius + 1.0)
        .ceil()
        .min(width as f32 - 1.0) as i32;
    let min_y = (start[1].min(end[1]) - radius - 1.0).floor().max(0.0) as i32;
    let max_y = (start[1].max(end[1]) + radius + 1.0)
        .ceil()
        .min(height as f32 - 1.0) as i32;

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let sample = [x as f32 + 0.5, y as f32 + 0.5];
            if distance_to_segment(sample, start, end) <= radius {
                blend_pixel(rgba, width, x as u32, y as u32, color);
            }
        }
    }
}

fn point_in_convex_polygon(point: [f32; 2], polygon: &[[f32; 2]]) -> bool {
    let mut sign = 0.0f32;
    for index in 0..polygon.len() {
        let a = polygon[index];
        let b = polygon[(index + 1) % polygon.len()];
        let cross = (b[0] - a[0]) * (point[1] - a[1]) - (b[1] - a[1]) * (point[0] - a[0]);
        if cross.abs() <= f32::EPSILON {
            continue;
        }
        if sign == 0.0 {
            sign = cross.signum();
        } else if cross.signum() != sign.signum() {
            return false;
        }
    }
    true
}

fn distance_to_segment(point: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let ap = [point[0] - a[0], point[1] - a[1]];
    let ab_len_sq = ab[0] * ab[0] + ab[1] * ab[1];
    if ab_len_sq <= f32::EPSILON {
        return ((point[0] - a[0]).powi(2) + (point[1] - a[1]).powi(2)).sqrt();
    }
    let t = ((ap[0] * ab[0] + ap[1] * ab[1]) / ab_len_sq).clamp(0.0, 1.0);
    let closest = [a[0] + ab[0] * t, a[1] + ab[1] * t];
    ((point[0] - closest[0]).powi(2) + (point[1] - closest[1]).powi(2)).sqrt()
}

fn blend_pixel(rgba: &mut [u8], width: u32, x: u32, y: u32, color: [u8; 4]) {
    let index = ((y * width + x) * 4) as usize;
    let src_alpha = color[3] as f32 / 255.0;
    let dst_alpha = rgba[index + 3] as f32 / 255.0;
    let out_alpha = src_alpha + dst_alpha * (1.0 - src_alpha);
    if out_alpha <= f32::EPSILON {
        return;
    }

    for channel in 0..3 {
        let src = color[channel] as f32 / 255.0;
        let dst = rgba[index + channel] as f32 / 255.0;
        let out = (src * src_alpha + dst * dst_alpha * (1.0 - src_alpha)) / out_alpha;
        rgba[index + channel] = (out.clamp(0.0, 1.0) * 255.0).round() as u8;
    }
    rgba[index + 3] = (out_alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
}
