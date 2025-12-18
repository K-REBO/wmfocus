// タイル位置にボックス描画テスト
// このテストは、Hyprlandから取得した各タイルの位置に
// 小さなボックスを描画して、位置情報が正しく取得できることを検証します。
//
// 実行方法:
//   cargo run --bin test_draw_boxes --features hyprland,wayland
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
    zwlr_layer_surface_v1::{self, Anchor},
};

struct AppState {
    surfaces: Vec<SurfaceData>,
    configured_count: usize,
}

struct SurfaceData {
    surface: wl_surface::WlSurface,
    layer_surface: zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    #[allow(dead_code)]
    x: i32,
    #[allow(dead_code)]
    y: i32,
    width: i32,
    height: i32,
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

impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, usize> for AppState {
    fn event(
        state: &mut Self,
        layer_surface: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        _idx: &usize,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zwlr_layer_surface_v1::Event::Configure { serial, .. } = event {
            layer_surface.ack_configure(serial);
            state.configured_count += 1;
        }
    }
}

fn create_solid_buffer(
    shm: &wl_shm::WlShm,
    qh: &QueueHandle<AppState>,
    width: i32,
    height: i32,
    color_argb: u32,
) -> Result<wl_buffer::WlBuffer> {
    let stride = width * 4;
    let size = stride * height;

    let temp_file = tempfile::tempfile().context("一時ファイルの作成に失敗")?;
    temp_file.set_len(size as u64).context("ファイルサイズの設定に失敗")?;

    let mut mmap = unsafe {
        memmap2::MmapMut::map_mut(&temp_file).context("mmapに失敗")?
    };

    // ARGB8888形式で塗りつぶし
    for pixel in mmap.chunks_exact_mut(4) {
        pixel[0] = (color_argb & 0xFF) as u8;         // B
        pixel[1] = ((color_argb >> 8) & 0xFF) as u8;  // G
        pixel[2] = ((color_argb >> 16) & 0xFF) as u8; // R
        pixel[3] = ((color_argb >> 24) & 0xFF) as u8; // A
    }

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

fn main() -> Result<()> {
    println!("=== タイル位置ボックス描画テスト ===\n");

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

    for (i, client) in visible_clients.iter().enumerate() {
        println!("  タイル{}: {} - 位置({}, {}) サイズ{}x{}",
            i + 1, client.title, client.at.0, client.at.1,
            client.size.0, client.size.1);
    }

    if visible_clients.is_empty() {
        println!("\n⚠ 可視タイルがありません。テストを終了します。");
        return Ok(());
    }

    // Step 2: Waylandで各タイルの位置にボックスを描画
    println!("\n【Step 2】各タイルの左上に赤いボックスを描画");
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
        surfaces: Vec::new(),
        configured_count: 0,
    };

    // 各タイルの左上にボックスを作成
    let box_size = 100; // ボックスのサイズ

    for (i, client) in visible_clients.iter().enumerate() {
        let surface = compositor.create_surface(&qh, ());
        let layer_surface = layer_shell.get_layer_surface(
            &surface,
            None,
            Layer::Overlay,
            "wmfocus_box".to_string(),
            &qh,
            i,
        );

        // 位置とサイズを設定
        // アンカーを左上に設定して、marginで位置を指定
        layer_surface.set_anchor(Anchor::Top | Anchor::Left);
        layer_surface.set_size(box_size as u32, box_size as u32);
        layer_surface.set_margin(
            client.at.1 as i32, // top - 上端からの距離
            0,                  // right
            0,                  // bottom
            client.at.0 as i32, // left - 左端からの距離
        );

        surface.commit();

        state.surfaces.push(SurfaceData {
            surface,
            layer_surface,
            x: client.at.0 as i32,
            y: client.at.1 as i32,
            width: box_size,
            height: box_size,
        });
    }

    // 全サーフェスがconfigureされるまで待機
    while state.configured_count < visible_clients.len() {
        event_queue.blocking_dispatch(&mut state)?;
    }

    println!("✓ {}個のLayer Surfaceを作成", visible_clients.len());

    // バッファを描画
    for surface_data in &state.surfaces {
        // 赤色の半透明ボックス (ARGB: 0xCCFF0000)
        let buffer = create_solid_buffer(
            &shm,
            &qh,
            surface_data.width,
            surface_data.height,
            0xCCFF0000,
        )?;

        surface_data.surface.attach(Some(&buffer), 0, 0);
        surface_data.surface.commit();
    }

    event_queue.roundtrip(&mut state)?;

    println!("✓ 各タイルの左上に赤いボックスを描画");
    println!("\n3秒間表示します...");

    std::thread::sleep(std::time::Duration::from_secs(3));

    // クリーンアップ
    for surface_data in state.surfaces {
        surface_data.layer_surface.destroy();
        surface_data.surface.destroy();
    }

    println!("\n✓ テスト完了");
    println!("\n=== 全テスト完了 ===");

    Ok(())
}
