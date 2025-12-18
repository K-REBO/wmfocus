// Wayland入力処理動作テスト（Keyboard + xkbcommon）
// このテストは、Waylandでキーボード入力を取得し、
// xkbcommonでキーシンボルに変換できることを検証します。
//
// 実行方法:
//   cargo run --bin test_wayland_input --features wayland
//
// 前提条件:
//   - Waylandコンポジタ（Hyprland）が起動していること
//
// 操作方法:
//   - 任意のキーを押すとキー情報が表示されます
//   - Escapeキーで終了します

use anyhow::{Context, Result};
use std::os::fd::AsFd;

// Waylandクライアントライブラリ
use wayland_client::{
    Connection, Dispatch, QueueHandle,
    protocol::{wl_compositor, wl_shm, wl_surface, wl_registry, wl_seat, wl_keyboard},
    globals::{registry_queue_init, GlobalListContents},
};

// Layer Shellプロトコル
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::{self, ZwlrLayerShellV1},
    zwlr_layer_surface_v1::{self, ZwlrLayerSurfaceV1, Anchor, KeyboardInteractivity},
};

// xkbcommon
use xkbcommon::xkb;

fn main() -> Result<()> {
    println!("=== Wayland 入力処理テスト ===\n");

    // テスト1: キーボード入力テスト
    println!("【テスト1】キーボード入力テスト");
    println!("{}", "-".repeat(50));
    test_keyboard_input()?;
    println!();

    println!("=== 全テスト完了 ===");
    Ok(())
}

/// テスト1: キーボード入力テスト
fn test_keyboard_input() -> Result<()> {
    println!("キーボード入力を受け取ります。");
    println!("任意のキーを押してください。Escapeキーで終了します。\n");

    // Waylandコンポジタへの接続
    let conn = Connection::connect_to_env()
        .context("Waylandコンポジタへの接続に失敗")?;

    // イベントキューとグローバルの初期化
    let (globals, mut event_queue) = registry_queue_init::<AppState>(&conn)
        .context("グローバルレジストリの取得に失敗")?;

    let qh = event_queue.handle();

    // 必要なグローバルをバインド
    let compositor: wl_compositor::WlCompositor = globals
        .bind(&qh, 4..=6, ())
        .context("wl_compositorのバインドに失敗")?;

    let shm: wl_shm::WlShm = globals
        .bind(&qh, 1..=1, ())
        .context("wl_shmのバインドに失敗")?;

    let layer_shell: ZwlrLayerShellV1 = globals
        .bind(&qh, 1..=4, ())
        .context("zwlr_layer_shell_v1のバインドに失敗")?;

    let seat: wl_seat::WlSeat = globals
        .bind(&qh, 7..=9, ())
        .context("wl_seatのバインドに失敗")?;

    println!("✓ Waylandグローバルをバインド");

    // サーフェスの作成（キーボード入力を受け取るために必要）
    let surface = compositor.create_surface(&qh, ());
    let layer_surface = layer_shell.get_layer_surface(
        &surface,
        None,
        zwlr_layer_shell_v1::Layer::Overlay,
        "wmfocus_test_input".to_string(),
        &qh,
        (),
    );

    // Layer Surface設定（キーボード入力を排他的に受け取る）
    layer_surface.set_size(200, 100);
    layer_surface.set_anchor(Anchor::Top | Anchor::Right); // 右上に小さく表示
    layer_surface.set_keyboard_interactivity(KeyboardInteractivity::Exclusive); // 排他的キーボード入力
    layer_surface.set_exclusive_zone(-1);

    println!("✓ Layer Surface作成（exclusive keyboard）");

    surface.commit();

    // xkbcommonコンテキストの作成
    let xkb_context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);

    // アプリケーション状態の初期化
    let mut state = AppState::new(xkb_context);

    // 初期イベント処理
    event_queue.roundtrip(&mut state)?;

    // キーボードを取得
    let _keyboard = seat.get_keyboard(&qh, ());
    println!("✓ キーボードを取得");

    event_queue.roundtrip(&mut state)?;

    // 小さなバッファを作成して表示（入力受付中の視覚的フィードバック）
    let buffer = create_indicator_buffer(&shm, &qh, 200, 100)
        .context("インジケータバッファの作成に失敗")?;
    surface.attach(Some(&buffer), 0, 0);
    surface.damage_buffer(0, 0, 200, 100);
    surface.commit();

    event_queue.roundtrip(&mut state)?;

    println!("\n入力受付中（右上に小さなインジケータが表示されます）");
    println!("キーを押してください...\n");

    // イベントループ
    while !state.should_exit {
        event_queue.blocking_dispatch(&mut state)?;
    }

    println!("\n✓ テスト完了");
    println!("（キー入力が正しく受け取れました）");

    Ok(())
}

/// インジケータ用の小さなバッファを作成
fn create_indicator_buffer(
    shm: &wl_shm::WlShm,
    qh: &QueueHandle<AppState>,
    width: i32,
    height: i32,
) -> Result<wayland_client::protocol::wl_buffer::WlBuffer> {
    let stride = width * 4;
    let size = stride * height;

    let file = tempfile::tempfile()
        .context("一時ファイルの作成に失敗")?;

    nix::unistd::ftruncate(&file, size as i64)
        .context("ファイルサイズの設定に失敗")?;

    let mmap = unsafe {
        memmap2::MmapMut::map_mut(&file)
            .context("メモリマップに失敗")?
    };

    // 半透明の緑色（入力受付中を示す）
    let color: u32 = 0xA000FF00; // A=0xA0 (半透明), R=0x00, G=0xFF, B=0x00
    let pixels = unsafe {
        std::slice::from_raw_parts_mut(
            mmap.as_ptr() as *mut u32,
            (size / 4) as usize,
        )
    };

    for pixel in pixels.iter_mut() {
        *pixel = color;
    }

    let pool = shm.create_pool(file.as_fd(), size, qh, ());
    let buffer = pool.create_buffer(
        0,
        width,
        height,
        stride,
        wayland_client::protocol::wl_shm::Format::Argb8888,
        qh,
        (),
    );

    pool.destroy();
    Ok(buffer)
}

// アプリケーション状態
struct AppState {
    xkb_context: xkb::Context,
    xkb_state: Option<xkb::State>,
    should_exit: bool,
}

impl AppState {
    fn new(xkb_context: xkb::Context) -> Self {
        Self {
            xkb_context,
            xkb_state: None,
            should_exit: false,
        }
    }
}

// Waylandイベントディスパッチャの実装
impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_registry::WlRegistry,
        _event: wl_registry::Event,
        _data: &GlobalListContents,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<wl_compositor::WlCompositor, ()> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_compositor::WlCompositor,
        _event: wl_compositor::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<wl_surface::WlSurface, ()> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_surface::WlSurface,
        _event: wl_surface::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<wl_shm::WlShm, ()> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_shm::WlShm,
        _event: wl_shm::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<wayland_client::protocol::wl_shm_pool::WlShmPool, ()> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &wayland_client::protocol::wl_shm_pool::WlShmPool,
        _event: wayland_client::protocol::wl_shm_pool::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<wayland_client::protocol::wl_buffer::WlBuffer, ()> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &wayland_client::protocol::wl_buffer::WlBuffer,
        _event: wayland_client::protocol::wl_buffer::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<ZwlrLayerShellV1, ()> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &ZwlrLayerShellV1,
        _event: zwlr_layer_shell_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<ZwlrLayerSurfaceV1, ()> for AppState {
    fn event(
        _state: &mut Self,
        proxy: &ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_layer_surface_v1::Event::Configure { serial, .. } => {
                proxy.ack_configure(serial);
            }
            zwlr_layer_surface_v1::Event::Closed => {}
            _ => {}
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_seat::WlSeat,
        _event: wl_seat::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for AppState {
    fn event(
        state: &mut Self,
        _proxy: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_keyboard::Event::Keymap { format, fd, size } => {
                if format == wayland_client::WEnum::Value(wl_keyboard::KeymapFormat::XkbV1) {
                    // キーマップを読み込む
                    let keymap = unsafe {
                        let ptr = nix::sys::mman::mmap(
                            None,
                            std::num::NonZeroUsize::new(size as usize).unwrap(),
                            nix::sys::mman::ProtFlags::PROT_READ,
                            nix::sys::mman::MapFlags::MAP_PRIVATE,
                            fd.as_fd(),
                            0,
                        ).expect("mmapに失敗");

                        let slice = std::slice::from_raw_parts(ptr.as_ptr() as *const u8, size as usize - 1);
                        let keymap_str = std::str::from_utf8_unchecked(slice);
                        let keymap = xkb::Keymap::new_from_string(
                            &state.xkb_context,
                            keymap_str.to_string(),
                            xkb::KEYMAP_FORMAT_TEXT_V1,
                            xkb::KEYMAP_COMPILE_NO_FLAGS,
                        ).expect("キーマップの作成に失敗");

                        nix::sys::mman::munmap(ptr, size as usize).expect("munmapに失敗");
                        keymap
                    };

                    state.xkb_state = Some(xkb::State::new(&keymap));
                    println!("✓ xkbcommonキーマップをロード");
                }
            }

            wl_keyboard::Event::Key { key, state: key_state, .. } => {
                if let Some(xkb_state) = &mut state.xkb_state {
                    let keycode = key + 8; // Waylandキーコード → xkbキーコード

                    if let wayland_client::WEnum::Value(wl_keyboard::KeyState::Pressed) = key_state {
                        // キーシンボルを取得
                        let keysym = xkb_state.key_get_one_sym(xkb::Keycode::from(keycode));
                        let keysym_name = xkb::keysym_get_name(keysym);

                        println!("キー押下: {} (keycode: {}, keysym: {:?})",
                            keysym_name, keycode, keysym);

                        // Escapeキーで終了
                        if keysym == xkb::keysyms::KEY_Escape.into() {
                            println!("\nEscapeキーが押されました。終了します。");
                            state.should_exit = true;
                        }
                    }
                }
            }

            wl_keyboard::Event::Modifiers { mods_depressed, mods_latched, mods_locked, group, .. } => {
                if let Some(xkb_state) = &mut state.xkb_state {
                    xkb_state.update_mask(mods_depressed, mods_latched, mods_locked, 0, 0, group);
                }
            }

            _ => {}
        }
    }
}
