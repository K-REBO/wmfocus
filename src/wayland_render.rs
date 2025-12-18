use anyhow::{Context, Result};
use log::info;
use std::collections::HashMap;
use std::os::fd::AsFd;

use hyprland::prelude::*;

use wayland_client::{
    protocol::{wl_buffer, wl_compositor, wl_keyboard, wl_registry, wl_seat, wl_shm, wl_shm_pool, wl_surface},
    globals::{registry_queue_init, GlobalListContents},
    Connection, Dispatch, QueueHandle,
};

use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::{self, Layer},
    zwlr_layer_surface_v1::{self, Anchor, KeyboardInteractivity},
};

use crate::{args::AppConfig, DesktopWindow};

pub struct WaylandRenderer {
    app_config: AppConfig,
}

struct RenderState {
    _compositor: wl_compositor::WlCompositor,
    _shm: wl_shm::WlShm,
    _layer_shell: zwlr_layer_shell_v1::ZwlrLayerShellV1,
    _seat: Option<wl_seat::WlSeat>,
    configured: bool,
    keyboard_state: Option<KeyboardState>,
    pressed_keys: String,
    should_exit: bool,
}

struct KeyboardState {
    xkb_context: xkbcommon::xkb::Context,
    xkb_state: Option<xkbcommon::xkb::State>,
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for RenderState {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<wl_compositor::WlCompositor, ()> for RenderState {
    fn event(
        _: &mut Self,
        _: &wl_compositor::WlCompositor,
        _: wl_compositor::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<wl_surface::WlSurface, ()> for RenderState {
    fn event(
        _: &mut Self,
        _: &wl_surface::WlSurface,
        _: wl_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<wl_shm::WlShm, ()> for RenderState {
    fn event(
        _: &mut Self,
        _: &wl_shm::WlShm,
        _: wl_shm::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<wl_shm_pool::WlShmPool, ()> for RenderState {
    fn event(
        _: &mut Self,
        _: &wl_shm_pool::WlShmPool,
        _: wl_shm_pool::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<wl_buffer::WlBuffer, ()> for RenderState {
    fn event(
        _: &mut Self,
        _: &wl_buffer::WlBuffer,
        _: wl_buffer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<zwlr_layer_shell_v1::ZwlrLayerShellV1, ()> for RenderState {
    fn event(
        _: &mut Self,
        _: &zwlr_layer_shell_v1::ZwlrLayerShellV1,
        _: zwlr_layer_shell_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, ()> for RenderState {
    fn event(
        state: &mut Self,
        layer_surface: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zwlr_layer_surface_v1::Event::Configure { serial, .. } = event {
            layer_surface.ack_configure(serial);
            state.configured = true;
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for RenderState {
    fn event(
        _: &mut Self,
        _: &wl_seat::WlSeat,
        _: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for RenderState {
    fn event(
        state: &mut Self,
        _keyboard: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use xkbcommon::xkb;

        match event {
            wl_keyboard::Event::Keymap { format, fd, size } => {
                if format == wayland_client::WEnum::Value(wl_keyboard::KeymapFormat::XkbV1) {
                    let keymap_data = unsafe {
                        let ptr = nix::sys::mman::mmap(
                            None,
                            std::num::NonZeroUsize::new(size as usize).unwrap(),
                            nix::sys::mman::ProtFlags::PROT_READ,
                            nix::sys::mman::MapFlags::MAP_PRIVATE,
                            fd.as_fd(),
                            0,
                        )
                        .expect("mmap failed");

                        let slice = std::slice::from_raw_parts(ptr.as_ptr() as *const u8, size as usize - 1);
                        let keymap_str = std::str::from_utf8_unchecked(slice);
                        let result = keymap_str.to_string();

                        nix::sys::mman::munmap(ptr, size as usize).expect("munmap failed");
                        result
                    };

                    if let Some(kb_state) = &mut state.keyboard_state {
                        let keymap = xkb::Keymap::new_from_string(
                            &kb_state.xkb_context,
                            keymap_data,
                            xkb::KEYMAP_FORMAT_TEXT_V1,
                            xkb::KEYMAP_COMPILE_NO_FLAGS,
                        )
                        .expect("Failed to create keymap");

                        kb_state.xkb_state = Some(xkb::State::new(&keymap));
                    }
                }
            }

            wl_keyboard::Event::Key { key, state: key_state, .. } => {
                if let Some(kb_state) = &mut state.keyboard_state {
                    if let Some(xkb_state) = &mut kb_state.xkb_state {
                        let keycode = key + 8; // Wayland to xkb conversion

                        if let wayland_client::WEnum::Value(wl_keyboard::KeyState::Pressed) = key_state {
                            let keysym = xkb_state.key_get_one_sym(xkb::Keycode::from(keycode));
                            let keysym_name = xkb::keysym_get_name(keysym);

                            // Handle escape key
                            if keysym == xkb::keysyms::KEY_Escape.into() {
                                state.should_exit = true;
                                return;
                            }

                            // Handle backspace
                            if keysym == xkb::keysyms::KEY_BackSpace.into() {
                                state.pressed_keys.pop();
                                return;
                            }

                            // Add printable characters to pressed_keys
                            if keysym_name.len() == 1 {
                                state.pressed_keys.push_str(&keysym_name.to_lowercase());
                            }
                        }
                    }
                }
            }

            wl_keyboard::Event::Modifiers { mods_depressed, mods_latched, mods_locked, group, .. } => {
                if let Some(kb_state) = &mut state.keyboard_state {
                    if let Some(xkb_state) = &mut kb_state.xkb_state {
                        xkb_state.update_mask(mods_depressed, mods_latched, mods_locked, 0, 0, group);
                    }
                }
            }

            _ => {}
        }
    }
}

impl WaylandRenderer {
    pub fn new(app_config: AppConfig) -> Result<Self> {
        Ok(Self {
            app_config,
        })
    }

    pub fn render_hints(
        &mut self,
        _desktop_windows: &[DesktopWindow],
        hints: &HashMap<String, &DesktopWindow>,
    ) -> Result<()> {
        info!("Rendering {} hints", hints.len());
        // Rendering will be done in wait_for_hint_selection
        Ok(())
    }

    pub fn wait_for_hint_selection<'a>(
        &mut self,
        hints: &HashMap<String, &'a DesktopWindow>,
    ) -> Result<Option<&'a DesktopWindow>> {
        let conn = Connection::connect_to_env().context("Failed to connect to Wayland")?;

        let (globals, mut event_queue) = registry_queue_init::<RenderState>(&conn)
            .context("Failed to get global registry")?;

        let qh = event_queue.handle();

        let compositor: wl_compositor::WlCompositor = globals
            .bind(&qh, 4..=6, ())
            .context("Failed to bind wl_compositor")?;

        let shm: wl_shm::WlShm = globals
            .bind(&qh, 1..=1, ())
            .context("Failed to bind wl_shm")?;

        let layer_shell: zwlr_layer_shell_v1::ZwlrLayerShellV1 = globals
            .bind(&qh, 1..=4, ())
            .context("Failed to bind zwlr_layer_shell_v1")?;

        let seat: wl_seat::WlSeat = globals
            .bind(&qh, 1..=7, ())
            .context("Failed to bind wl_seat")?;

        let mut state = RenderState {
            _compositor: compositor.clone(),
            _shm: shm.clone(),
            _layer_shell: layer_shell.clone(),
            _seat: Some(seat.clone()),
            configured: false,
            keyboard_state: Some(KeyboardState {
                xkb_context: xkbcommon::xkb::Context::new(xkbcommon::xkb::CONTEXT_NO_FLAGS),
                xkb_state: None,
            }),
            pressed_keys: String::new(),
            should_exit: false,
        };

        // Get screen dimensions from Hyprland
        let monitors = hyprland::data::Monitors::get().context("Failed to get monitors")?;
        let monitor_vec = monitors.to_vec();

        let screen_width = monitor_vec.iter()
            .map(|m| m.x + m.width as i32)
            .max()
            .unwrap_or(1920);
        let screen_height = monitor_vec.iter()
            .map(|m| m.y + m.height as i32)
            .max()
            .unwrap_or(1080);

        info!("Screen size: {}x{}", screen_width, screen_height);

        // Create full-screen overlay surface
        let surface = compositor.create_surface(&qh, ());
        let layer_surface = layer_shell.get_layer_surface(
            &surface,
            None,
            Layer::Overlay,
            "wmfocus".to_string(),
            &qh,
            (),
        );

        layer_surface.set_anchor(Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right);
        layer_surface.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        layer_surface.set_exclusive_zone(-1);

        surface.commit();

        // Wait for configure
        while !state.configured {
            event_queue.blocking_dispatch(&mut state)?;
        }

        // Get keyboard
        let _keyboard = seat.get_keyboard(&qh, ());

        // Create buffer with hints rendered
        let buffer = self.create_hints_buffer(&shm, &qh, screen_width, screen_height, hints)?;

        surface.attach(Some(&buffer), 0, 0);
        surface.damage_buffer(0, 0, screen_width, screen_height);
        surface.commit();

        event_queue.roundtrip(&mut state)?;

        info!("Overlay displayed. Press hint keys or ESC to cancel.");

        // Event loop
        while !state.should_exit {
            event_queue.blocking_dispatch(&mut state)?;

            // Check if pressed_keys matches any hint
            if let Some(window) = hints.get(&state.pressed_keys) {
                info!("Hint '{}' selected", state.pressed_keys);
                return Ok(Some(window));
            }
        }

        Ok(None)
    }

    fn create_hints_buffer(
        &self,
        shm: &wl_shm::WlShm,
        qh: &QueueHandle<RenderState>,
        width: i32,
        height: i32,
        hints: &HashMap<String, &DesktopWindow>,
    ) -> Result<wl_buffer::WlBuffer> {
        let stride = width * 4;
        let size = stride * height;

        let temp_file = tempfile::tempfile().context("Failed to create temp file")?;
        temp_file.set_len(size as u64).context("Failed to set file size")?;

        let mut cairo_surface = cairo::ImageSurface::create(
            cairo::Format::ARgb32,
            width,
            height,
        )
        .context("Failed to create Cairo surface")?;

        {
            let cairo_context = cairo::Context::new(&cairo_surface)
                .context("Failed to create Cairo context")?;

            // Transparent background
            cairo_context.set_source_rgba(0.0, 0.0, 0.0, 0.0);
            cairo_context.paint().context("Failed to paint background")?;

            // Draw hint for each window
            for (hint, window) in hints {
                self.draw_hint(&cairo_context, hint, window)?;
            }
        }

        cairo_surface.flush();
        let cairo_data = cairo_surface.data().context("Failed to get Cairo data")?;

        // Copy to Wayland buffer
        let mut mmap = unsafe {
            memmap2::MmapMut::map_mut(&temp_file).context("mmap failed")?
        };

        mmap.copy_from_slice(&cairo_data);
        drop(mmap);

        let pool = shm.create_pool(temp_file.as_fd(), size, qh, ());
        let buffer = pool.create_buffer(
            0,
            width,
            height,
            stride,
            wl_shm::Format::Argb8888,
            qh,
            (),
        );

        pool.destroy();
        Ok(buffer)
    }

    fn draw_hint(&self, ctx: &cairo::Context, hint: &str, window: &DesktopWindow) -> Result<()> {
        let x = window.pos.0 as f64;
        let y = window.pos.1 as f64;

        // Set font first to get accurate text extents
        ctx.select_font_face(
            &self.app_config.font.font_family,
            cairo::FontSlant::Normal,
            cairo::FontWeight::Bold,
        );
        ctx.set_font_size(self.app_config.font.font_size);

        let text_extents = ctx.text_extents(hint)?;

        // Use margin from config
        let base_size = self.app_config.font.font_size;
        let margin = base_size * self.app_config.margin as f64;

        let rect_width = text_extents.width() + margin * 2.0;
        let rect_height = base_size + margin * 2.0;

        // Draw rounded rectangle background
        let bg = if window.is_focused {
            self.app_config.bg_color_current
        } else {
            self.app_config.bg_color
        };
        ctx.set_source_rgba(bg.0, bg.1, bg.2, bg.3);

        let radius = 5.0;
        let degrees = std::f64::consts::PI / 180.0;

        ctx.new_sub_path();
        ctx.arc(x + rect_width - radius, y + radius, radius, -90.0 * degrees, 0.0 * degrees);
        ctx.arc(x + rect_width - radius, y + rect_height - radius, radius, 0.0 * degrees, 90.0 * degrees);
        ctx.arc(x + radius, y + rect_height - radius, radius, 90.0 * degrees, 180.0 * degrees);
        ctx.arc(x + radius, y + radius, radius, 180.0 * degrees, 270.0 * degrees);
        ctx.close_path();
        ctx.fill()?;

        // Hint text
        let text = if window.is_focused {
            self.app_config.text_color_current
        } else {
            self.app_config.text_color
        };
        ctx.set_source_rgba(text.0, text.1, text.2, text.3);

        let text_x = x + margin;
        let text_y = y + margin + text_extents.height();
        ctx.move_to(text_x, text_y);
        ctx.show_text(hint)?;

        Ok(())
    }
}
