use super::utility_types::{OverlayProvider, OverlaysVisibilitySettings};
use crate::messages::prelude::*;

#[derive(ExtractField)]
pub struct OverlaysMessageContext<'a> {
	pub visibility_settings: OverlaysVisibilitySettings,
	pub ipp: &'a InputPreprocessorMessageHandler,
	pub device_pixel_ratio: f64,
}

#[derive(Debug, Clone, Default, ExtractField)]
pub struct OverlaysMessageHandler {
	pub overlay_providers: HashSet<OverlayProvider>,
	#[cfg(target_arch = "wasm32")]
	canvas: Option<web_sys::HtmlCanvasElement>,
	#[cfg(target_arch = "wasm32")]
	context: Option<web_sys::CanvasRenderingContext2d>,
}

#[message_handler_data]
impl MessageHandler<OverlaysMessage, OverlaysMessageContext<'_>> for OverlaysMessageHandler {
	fn process_message(&mut self, message: OverlaysMessage, responses: &mut VecDeque<Message>, context: OverlaysMessageContext) {
		let OverlaysMessageContext { visibility_settings, ipp, .. } = context;
		#[cfg(target_arch = "wasm32")]
		let device_pixel_ratio = context.device_pixel_ratio;

		match message {
			#[cfg(target_arch = "wasm32")]
			OverlaysMessage::Draw => {
				use super::utility_functions::overlay_canvas_element;
				use super::utility_types::OverlayContext;
				use glam::{DAffine2, DVec2};
				use wasm_bindgen::JsCast;

				let canvas = match &self.canvas {
					Some(canvas) => canvas,
					None => {
						let Some(new_canvas) = overlay_canvas_element() else { return };
						self.canvas.get_or_insert(new_canvas)
					}
				};

				let canvas_context = self.context.get_or_insert_with(|| {
					let canvas_context = canvas.get_context("2d").ok().flatten().expect("Failed to get canvas context");
					canvas_context.dyn_into().expect("Context should be a canvas 2d context")
				});

				let size = ipp.viewport_bounds.size().as_uvec2();

				let [a, b, c, d, e, f] = DAffine2::from_scale(DVec2::splat(device_pixel_ratio)).to_cols_array();
				let _ = canvas_context.set_transform(a, b, c, d, e, f);
				canvas_context.clear_rect(0., 0., ipp.viewport_bounds.size().x, ipp.viewport_bounds.size().y);
				let _ = canvas_context.reset_transform();

				if visibility_settings.all() {
					responses.add(DocumentMessage::GridOverlays(OverlayContext {
						render_context: canvas_context.clone(),
						size: size.as_dvec2(),
						device_pixel_ratio,
						visibility_settings: visibility_settings.clone(),
					}));
					for provider in &self.overlay_providers {
						responses.add(provider(OverlayContext {
							render_context: canvas_context.clone(),
							size: size.as_dvec2(),
							device_pixel_ratio,
							visibility_settings: visibility_settings.clone(),
						}));
					}
				}
			}
			#[cfg(not(target_arch = "wasm32"))]
			OverlaysMessage::Draw => {
				warn!("Cannot render overlays on non-Wasm targets.\n{responses:?} {visibility_settings:?} {ipp:?}",);
			}
			OverlaysMessage::AddProvider(message) => {
				self.overlay_providers.insert(message);
			}
			OverlaysMessage::RemoveProvider(message) => {
				self.overlay_providers.remove(&message);
			}
		}
	}

	advertise_actions!(OverlaysMessage;);
}
