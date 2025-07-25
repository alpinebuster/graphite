use std::fmt::Debug;
use std::process::exit;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use tracing_subscriber::EnvFilter;
use winit::event_loop::{EventLoop, EventLoopProxy};

mod cef;
use cef::Setup;

mod render;
use render::{FrameBuffer, GraphicsState};

mod app;
use app::WinitApp;

#[derive(Debug)]
pub(crate) enum CustomEvent {
	UiUpdate,
	ScheduleBrowserWork(Instant),
}

#[derive(Debug)]
pub(crate) struct WindowState {
	width: Option<usize>,
	height: Option<usize>,
	ui_frame_buffer: Option<FrameBuffer>,
	_viewport_frame_buffer: Option<FrameBuffer>,
	graphics_state: Option<GraphicsState>,
	event_loop_proxy: Option<EventLoopProxy<CustomEvent>>,
}

impl WindowState {
	fn new() -> Self {
		Self {
			width: None,
			height: None,
			ui_frame_buffer: None,
			_viewport_frame_buffer: None,
			graphics_state: None,
			event_loop_proxy: None,
		}
	}

	fn handle(self) -> WindowStateHandle {
		WindowStateHandle { inner: Arc::new(Mutex::new(self)) }
	}
}

pub(crate) struct WindowStateHandle {
	inner: Arc<Mutex<WindowState>>,
}

impl WindowStateHandle {
	fn with<'a, P>(&self, p: P) -> Result<(), PoisonError<MutexGuard<'a, WindowState>>>
	where
		P: FnOnce(&mut WindowState),
	{
		match self.inner.lock() {
			Ok(mut guard) => {
				p(&mut guard);
				Ok(())
			}
			Err(_) => todo!("not error handling yet"),
		}
	}
}

impl Clone for WindowStateHandle {
	fn clone(&self) -> Self {
		Self { inner: self.inner.clone() }
	}
}

#[derive(Clone)]
struct CefHandler {
	window_state: WindowStateHandle,
}

impl CefHandler {
	fn new(window_state: WindowStateHandle) -> Self {
		Self { window_state }
	}
}

impl cef::CefEventHandler for CefHandler {
	fn window_size(&self) -> cef::WindowSize {
		let mut w = 1;
		let mut h = 1;

		self.window_state
			.with(|s| {
				if let WindowState {
					width: Some(width),
					height: Some(height),
					..
				} = s
				{
					w = *width;
					h = *height;
				}
			})
			.unwrap();

		cef::WindowSize::new(w, h)
	}

	fn draw(&self, frame_buffer: FrameBuffer) -> bool {
		let mut correct_size = true;
		self.window_state
			.with(|s| {
				if let Some(event_loop_proxy) = &s.event_loop_proxy {
					let _ = event_loop_proxy.send_event(CustomEvent::UiUpdate);
				}
				if frame_buffer.width() != s.width.unwrap_or(1) || frame_buffer.height() != s.height.unwrap_or(1) {
					correct_size = false;
				} else {
					s.ui_frame_buffer = Some(frame_buffer);
				}
			})
			.unwrap();

		correct_size
	}

	fn schedule_cef_message_loop_work(&self, scheduled_time: std::time::Instant) {
		self.window_state
			.with(|s| {
				let Some(event_loop_proxy) = &mut s.event_loop_proxy else { return };
				let _ = event_loop_proxy.send_event(CustomEvent::ScheduleBrowserWork(scheduled_time));
			})
			.unwrap();
	}
}

fn main() {
	tracing_subscriber::fmt().with_env_filter(EnvFilter::from_default_env()).init();

	let cef_context = match cef::Context::<Setup>::new() {
		Ok(c) => c,
		Err(cef::SetupError::Subprocess) => exit(0),
		Err(cef::SetupError::SubprocessFailed(t)) => {
			tracing::error!("Subprocess of type {t} failed");
			exit(1);
		}
	};

	let window_state = WindowState::new().handle();

	window_state
		.with(|s| {
			s.width = Some(1200);
			s.height = Some(800);
		})
		.unwrap();

	let event_loop = EventLoop::<CustomEvent>::with_user_event().build().unwrap();

	window_state.with(|s| s.event_loop_proxy = Some(event_loop.create_proxy())).unwrap();

	let cef_context = match cef_context.init(CefHandler::new(window_state.clone())) {
		Ok(c) => c,
		Err(cef::InitError::InitializationFailed) => {
			tracing::error!("Cef initialization failed");
			exit(1);
		}
	};

	tracing::info!("Cef initialized successfully");

	let mut winit_app = WinitApp::new(window_state, cef_context);

	event_loop.run_app(&mut winit_app).unwrap();
}
