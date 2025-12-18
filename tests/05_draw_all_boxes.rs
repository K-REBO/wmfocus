// タイル位置に全ボックス描画テスト（画面全体サーフェス版）
// このテストは、画面全体を覆う1つの透明サーフェスを作成し、
// その上に各タイルの位置に応じてボックスを描画します。
//
// 実行方法:
//   cargo run --bin test_all_boxes --features hyprland,wayland
//
// 前提条件:
//   - Waylandコンポジタ（Hyprland）が起動していること
//   - 複数のタイルウィンドウが表示されていること
//
// 操作方法:
//   - 3秒間、各タイルの左上に赤いボックスが表示されます

use anyhow::{Context, Result};
use std::os::fd::AsFd;

// Hyprlandクライアントライブラリ
use hyprland::data::{Clients, Monitors};
use hyprland::prelude::*;

// Waylandクライアントライブラリ
use wayland_client::{
    Connection, Dispatch, QueueHandle,
    protocol::{wl_compositor, wl_registry, wl_shm, wl_buffer, wl_surface, wl_shm_pool},
    globals::{registry_queue_init, GlobalListContents},
};

use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::{self, Layer},
    zwlr_layer_surface_v1::{self, Anchor, KeyboardInteractivity},
};

struct AppState {
    configured: bool,
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for AppState {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<wl_compositor::WlCompositor, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &wl_compositor::WlCompositor,
        _: wl_compositor::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<wl_surface::WlSurface, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &wl_surface::WlSurface,
        _: wl_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<wl_shm::WlShm, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &wl_shm::WlShm,
        _: wl_shm::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<wl_shm_pool::WlShmPool, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &wl_shm_pool::WlShmPool,
        _: wl_shm_pool::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<wl_buffer::WlBuffer, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &wl_buffer::WlBuffer,
        _: wl_buffer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<zwlr_layer_shell_v1::ZwlrLayerShellV1, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &zwlr_layer_shell_v1::ZwlrLayerShellV1,
        _: zwlr_layer_shell_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {}
}

impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, ()> for AppState {
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

#[derive(Clone)]
struct TileInfo {
    x: i32,
    y: i32,
    #[allow(dead_code)]
    width: i32,
    #[allow(dead_code)]
    height: i32,
}

fn create_buffer_with_boxes(
    shm: &wl_shm::WlShm,
    qh: &QueueHandle<AppState>,
    screen_width: i32,
    screen_height: i32,
    tiles: &[TileInfo],
) -> Result<wl_buffer::WlBuffer> {
    let stride = screen_width * 4;
    let size = stride * screen_height;

    let temp_file = tempfile::tempfile().context("一時ファイルの作成に失敗")?;
    temp_file.set_len(size as u64).context("ファイルサイズの設定に失敗")?;

    let mut mmap = unsafe {
        memmap2::MmapMut::map_mut(&temp_file).context("mmapに失敗")?
    };

    // 全体を透明に初期化 (ARGB: 0x00000000)
    for pixel in mmap.chunks_exact_mut(4) {
        pixel[0] = 0; // B
        pixel[1] = 0; // G
        pixel[2] = 0; // R
        pixel[3] = 0; // A (完全透明)
    }

    // 各タイルの位置に赤いボックスを描画
    let box_size = 100;
    let box_color_argb = 0xCCFF0000u32; // 半透明の赤

    for tile in tiles {
        let box_x = tile.x;
        let box_y = tile.y;

        // ボックスの範囲をクリップ
        let x_start = box_x.max(0);
        let x_end = (box_x + box_size).min(screen_width);
        let y_start = box_y.max(0);
        let y_end = (box_y + box_size).min(screen_height);

        for y in y_start..y_end {
            for x in x_start..x_end {
                let offset = ((y * screen_width + x) * 4) as usize;
                if offset + 3 < mmap.len() {
                    mmap[offset + 0] = (box_color_argb & 0xFF) as u8;         // B
                    mmap[offset + 1] = ((box_color_argb >> 8) & 0xFF) as u8;  // G
                    mmap[offset + 2] = ((box_color_argb >> 16) & 0xFF) as u8; // R
                    mmap[offset + 3] = ((box_color_argb >> 24) & 0xFF) as u8; // A
                }
            }
        }
    }

    drop(mmap);

    let pool = shm.create_pool(temp_file.as_fd(), size, qh, ());
    let buffer = pool.create_buffer(
        0,
        screen_width,
        screen_height,
        stride,
        wl_shm::Format::Argb8888,
        qh,
        (),
    );

    pool.destroy();
    Ok(buffer)
}

fn main() -> Result<()> {
    println!("=== タイル位置ボックス描画テスト（画面全体版） ===\n");

    // Step 1: Hyprland IPCで可視タイルを取得
    println!("【Step 1】Hyprlandから可視タイル情報を取得");
    println!("--------------------------------------------------");

    let clients = Clients::get().context("ウィンドウリストの取得に失敗")?;
    let client_vec = clients.to_vec();

    let monitors = Monitors::get().context("モニター情報の取得に失敗")?;
    let monitor_vec = monitors.to_vec();

    // アクティブなワークスペースIDを取得
    let visible_workspace_ids: Vec<i32> = monitor_vec
        .iter()
        .map(|m| m.active_workspace.id)
        .collect();

    // 可視タイルのみをフィルタ
    let visible_clients: Vec<_> = client_vec
        .iter()
        .filter(|c| visible_workspace_ids.contains(&c.workspace.id))
        .collect();

    println!("✓ 可視タイル数: {}", visible_clients.len());

    let tiles: Vec<TileInfo> = visible_clients
        .iter()
        .map(|c| {
            println!("  タイル: {} - 位置({}, {}) サイズ{}x{}",
                c.title, c.at.0, c.at.1, c.size.0, c.size.1);
            TileInfo {
                x: c.at.0 as i32,
                y: c.at.1 as i32,
                width: c.size.0 as i32,
                height: c.size.1 as i32,
            }
        })
        .collect();

    if tiles.is_empty() {
        println!("\n⚠ 可視タイルがありません。テストを終了します。");
        return Ok(());
    }

    // モニターの解像度を取得
    let screen_width = monitor_vec.iter().map(|m| m.width).max().unwrap_or(1920) as i32;
    let screen_height = monitor_vec.iter().map(|m| m.height).max().unwrap_or(1080) as i32;
    println!("\n画面サイズ: {}x{}", screen_width, screen_height);

    // Step 2: Waylandで画面全体を覆うサーフェスに全ボックスを描画
    println!("\n【Step 2】画面全体サーフェスに全ボックスを描画");
    println!("--------------------------------------------------");

    let conn = Connection::connect_to_env().context("Waylandへの接続に失敗")?;

    let (globals, mut event_queue) = registry_queue_init::<AppState>(&conn)
        .context("グローバルレジストリの取得に失敗")?;

    let qh = event_queue.handle();

    let compositor: wl_compositor::WlCompositor = globals
        .bind(&qh, 4..=6, ())
        .context("wl_compositorのバインドに失敗")?;

    let shm: wl_shm::WlShm = globals
        .bind(&qh, 1..=1, ())
        .context("wl_shmのバインドに失敗")?;

    let layer_shell: zwlr_layer_shell_v1::ZwlrLayerShellV1 = globals
        .bind(&qh, 1..=4, ())
        .context("zwlr_layer_shell_v1のバインドに失敗")?;

    println!("✓ Waylandグローバルをバインド");

    let mut state = AppState {
        configured: false,
    };

    // 画面全体を覆うサーフェスを作成
    let surface = compositor.create_surface(&qh, ());
    let layer_surface = layer_shell.get_layer_surface(
        &surface,
        None,
        Layer::Overlay,
        "wmfocus_overlay".to_string(),
        &qh,
        (),
    );

    // 画面全体を覆う設定
    layer_surface.set_anchor(Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right);
    layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);
    layer_surface.set_exclusive_zone(-1);

    surface.commit();

    // Configureイベントを待つ
    while !state.configured {
        event_queue.blocking_dispatch(&mut state)?;
    }

    println!("✓ 画面全体Layer Surfaceを作成");

    // 全ボックスを含むバッファを作成
    let buffer = create_buffer_with_boxes(&shm, &qh, screen_width, screen_height, &tiles)?;

    println!("✓ {}個のボックスを描画したバッファを作成", tiles.len());

    // バッファをアタッチして表示
    surface.attach(Some(&buffer), 0, 0);
    surface.damage_buffer(0, 0, screen_width, screen_height);
    surface.commit();

    event_queue.roundtrip(&mut state)?;

    println!("✓ ボックスを表示");
    println!("\n3秒間表示します...");

    std::thread::sleep(std::time::Duration::from_secs(3));

    // クリーンアップ
    layer_surface.destroy();
    surface.destroy();

    println!("\n✓ テスト完了");
    println!("\n=== 全テスト完了 ===");

    Ok(())
}
