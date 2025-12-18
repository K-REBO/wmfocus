// Waylandレンダリング動作テスト（Layer Shell + Cairo）
// このテストは、Wayland Layer Shellプロトコルを使って
// オーバーレイサーフェスを作成し、Cairo描画を行うことを検証します。
//
// 実行方法:
//   cargo run --bin test_wayland_render --features wayland
//
// 前提条件:
//   - Waylandコンポジタ（Hyprland）が起動していること
//   - WAYLAND_DISPLAY環境変数が設定されていること

use anyhow::{Context, Result, bail};
use std::os::fd::AsFd;
use std::time::Duration;

// Waylandクライアントライブラリ
use wayland_client::{
    Connection, Dispatch, QueueHandle,
    protocol::{wl_compositor, wl_shm, wl_shm_pool, wl_surface, wl_buffer, wl_output, wl_registry},
    globals::{registry_queue_init, GlobalListContents},
};

// Layer Shellプロトコル
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::{self, ZwlrLayerShellV1},
    zwlr_layer_surface_v1::{self, ZwlrLayerSurfaceV1, Anchor, KeyboardInteractivity},
};

fn main() -> Result<()> {
    println!("=== Wayland Layer Shell レンダリングテスト ===\n");

    // テスト1: Wayland接続
    println!("【テスト1】Wayland接続テスト");
    println!("{}", "-".repeat(50));
    test_wayland_connection()?;
    println!();

    // テスト2: Layer Shell基本テスト
    println!("【テスト2】Layer Shell基本テスト（単色矩形）");
    println!("{}", "-".repeat(50));
    test_layer_shell()?;
    println!();

    // テスト3: Cairo文字描画テスト
    println!("【テスト3】Cairo文字描画テスト");
    println!("{}", "-".repeat(50));
    test_cairo_text_rendering()?;
    println!();

    println!("=== 全テスト完了 ===");
    Ok(())
}

/// テスト1: Wayland接続テスト
fn test_wayland_connection() -> Result<()> {
    // WAYLAND_DISPLAY環境変数の確認
    let display_name = std::env::var("WAYLAND_DISPLAY")
        .unwrap_or_else(|_| "wayland-0".to_string());
    println!("WAYLAND_DISPLAY: {}", display_name);

    // Waylandコンポジタへの接続
    let conn = Connection::connect_to_env()
        .context("Waylandコンポジタへの接続に失敗")?;
    println!("✓ Waylandコンポジタに接続成功");

    // グローバルレジストリの取得
    let (globals, _) = registry_queue_init::<AppState>(&conn)
        .context("グローバルレジストリの取得に失敗")?;

    println!("\n利用可能なWaylandグローバル:");

    // 各グローバルの確認
    let has_compositor = globals.contents().clone_list().iter()
        .any(|g| g.interface == "wl_compositor");
    let has_shm = globals.contents().clone_list().iter()
        .any(|g| g.interface == "wl_shm");
    let has_layer_shell = globals.contents().clone_list().iter()
        .any(|g| g.interface == "zwlr_layer_shell_v1");

    if has_compositor {
        println!("  ✓ wl_compositor");
    } else {
        bail!("wl_compositorが利用できません");
    }

    if has_shm {
        println!("  ✓ wl_shm");
    } else {
        bail!("wl_shmが利用できません");
    }

    if has_layer_shell {
        println!("  ✓ zwlr_layer_shell_v1 (Layer Shell)");
    } else {
        bail!("zwlr_layer_shell_v1が利用できません（Layer Shellプロトコルが必要）");
    }

    println!("\n✓ 必要なWaylandプロトコルがすべて利用可能");

    Ok(())
}

/// テスト2: Layer Shell基本テスト
/// 単一の透明オーバーレイを画面中央に表示
fn test_layer_shell() -> Result<()> {
    println!("単一の透明オーバーレイを3秒間表示します...\n");

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
    println!("✓ wl_compositorをバインド");

    let shm: wl_shm::WlShm = globals
        .bind(&qh, 1..=1, ())
        .context("wl_shmのバインドに失敗")?;
    println!("✓ wl_shmをバインド");

    let layer_shell: ZwlrLayerShellV1 = globals
        .bind(&qh, 1..=4, ())
        .context("zwlr_layer_shell_v1のバインドに失敗")?;
    println!("✓ zwlr_layer_shell_v1をバインド");

    // サーフェスの作成
    let surface = compositor.create_surface(&qh, ());
    println!("✓ wl_surfaceを作成");

    // Layer Surfaceの作成
    let layer_surface = layer_shell.get_layer_surface(
        &surface,
        None, // 特定のoutputを指定しない（デフォルト）
        zwlr_layer_shell_v1::Layer::Overlay, // 最前面
        "wmfocus_test".to_string(),
        &qh,
        (),
    );
    println!("✓ Layer Surfaceを作成");

    // Layer Surfaceの設定
    let width = 400;
    let height = 300;

    layer_surface.set_size(width, height);
    layer_surface.set_anchor(Anchor::empty()); // アンカーなし（中央配置）
    layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);
    layer_surface.set_exclusive_zone(-1); // 他のウィンドウに影響を与えない

    println!("✓ Layer Surface設定完了 ({}x{})", width, height);

    // 初期コミット（サーフェスの設定を確定）
    surface.commit();

    // イベントループで設定を待機
    event_queue.blocking_dispatch(&mut AppState::new())?;

    println!("✓ サーフェス設定を送信");

    // 共有メモリバッファの作成
    let buffer = create_shm_buffer(&shm, &qh, width as i32, height as i32)
        .context("共有メモリバッファの作成に失敗")?;
    println!("✓ 共有メモリバッファを作成");

    // バッファをサーフェスにアタッチ
    surface.attach(Some(&buffer), 0, 0);
    surface.damage_buffer(0, 0, width as i32, height as i32);
    surface.commit();

    println!("✓ バッファをアタッチしてコミット");

    // イベントループを実行
    println!("\nオーバーレイを表示中...");
    println!("（画面中央に{}x{}の半透明な灰色の矩形が表示されるはずです）", width, height);

    // 初期イベント処理
    let mut state = AppState::new();
    event_queue.roundtrip(&mut state)?;

    println!("\n3秒間表示...");
    std::thread::sleep(Duration::from_secs(3));

    println!("\n✓ テスト完了");
    println!("（画面に半透明な灰色の矩形が表示されたことを確認してください）");

    Ok(())
}

/// テスト3: Cairo文字描画テスト
/// Cairoを使ってテキストを描画し、Waylandで表示
fn test_cairo_text_rendering() -> Result<()> {
    println!("Cairoでテキストを描画したオーバーレイを3秒間表示します...\n");

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

    // サーフェスの作成
    let surface = compositor.create_surface(&qh, ());
    let layer_surface = layer_shell.get_layer_surface(
        &surface,
        None,
        zwlr_layer_shell_v1::Layer::Overlay,
        "wmfocus_test_cairo".to_string(),
        &qh,
        (),
    );

    // サイズ設定
    let width = 400;
    let height = 300;

    layer_surface.set_size(width, height);
    layer_surface.set_anchor(Anchor::empty());
    layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);
    layer_surface.set_exclusive_zone(-1);

    println!("✓ Layer Surface設定完了 ({}x{})", width, height);

    surface.commit();

    // イベントループで設定を待機
    event_queue.blocking_dispatch(&mut AppState::new())?;

    // Cairoで文字を描画したバッファを作成
    let buffer = create_cairo_text_buffer(&shm, &qh, width as i32, height as i32, "wmfocus")
        .context("Cairo文字描画バッファの作成に失敗")?;
    println!("✓ Cairo文字描画バッファを作成");

    // バッファをサーフェスにアタッチ
    surface.attach(Some(&buffer), 0, 0);
    surface.damage_buffer(0, 0, width as i32, height as i32);
    surface.commit();

    println!("✓ バッファをアタッチしてコミット");

    // イベント処理
    let mut state = AppState::new();
    event_queue.roundtrip(&mut state)?;

    println!("\nオーバーレイを表示中...");
    println!("（画面中央に\"wmfocus\"という文字が表示されるはずです）");
    println!("\n3秒間表示...");
    std::thread::sleep(Duration::from_secs(3));

    println!("\n✓ テスト完了");
    println!("（Cairoで描画されたテキストが表示されたことを確認してください）");

    Ok(())
}

/// 共有メモリバッファを作成
fn create_shm_buffer(
    shm: &wl_shm::WlShm,
    qh: &QueueHandle<AppState>,
    width: i32,
    height: i32,
) -> Result<wl_buffer::WlBuffer> {
    let stride = width * 4; // ARGB8888 = 4 bytes per pixel
    let size = stride * height;

    // 一時ファイルを作成（共有メモリ用）
    let file = tempfile::tempfile()
        .context("一時ファイルの作成に失敗")?;

    // ファイルサイズを設定
    nix::unistd::ftruncate(&file, size as i64)
        .context("ファイルサイズの設定に失敗")?;

    // メモリマップ
    let mmap = unsafe {
        memmap2::MmapMut::map_mut(&file)
            .context("メモリマップに失敗")?
    };

    // バッファに半透明グレーを描画（ARGB8888形式）
    // フォーマット: 0xAARRGGBB
    let color: u32 = 0x80808080; // 半透明グレー (A=0x80, R=0x80, G=0x80, B=0x80)

    let pixels = unsafe {
        std::slice::from_raw_parts_mut(
            mmap.as_ptr() as *mut u32,
            (size / 4) as usize,
        )
    };

    for pixel in pixels.iter_mut() {
        *pixel = color;
    }

    // 共有メモリプールを作成
    let pool = shm.create_pool(
        file.as_fd(),
        size,
        qh,
        (),
    );

    // バッファを作成
    let buffer = pool.create_buffer(
        0, // offset
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

/// Cairoでテキストを描画した共有メモリバッファを作成
fn create_cairo_text_buffer(
    shm: &wl_shm::WlShm,
    qh: &QueueHandle<AppState>,
    width: i32,
    height: i32,
    text: &str,
) -> Result<wl_buffer::WlBuffer> {
    let stride = width * 4; // ARGB8888 = 4 bytes per pixel
    let size = stride * height;

    // Cairo ImageSurfaceを作成
    let mut cairo_surface = cairo::ImageSurface::create(
        cairo::Format::ARgb32,
        width,
        height,
    )
    .context("Cairo ImageSurfaceの作成に失敗")?;

    // スコープ内でCairo描画を行う（コンテキストをドロップするため）
    {
        let cairo_context = cairo::Context::new(&cairo_surface)
            .context("Cairo Contextの作成に失敗")?;

        // 背景を半透明の暗い色で塗りつぶし
        cairo_context.set_source_rgba(0.1, 0.1, 0.1, 0.9); // 暗い背景、90%不透明
        cairo_context.paint().context("背景描画に失敗")?;

        // テキストを描画
        cairo_context.select_font_face(
            "Sans",
            cairo::FontSlant::Normal,
            cairo::FontWeight::Bold,
        );
        cairo_context.set_font_size(72.0);

        // テキストのサイズを測定して中央配置
        let extents = cairo_context.text_extents(text)
            .context("テキストサイズ測定に失敗")?;

        let x = (f64::from(width) - extents.width()) / 2.0 - extents.x_bearing();
        let y = (f64::from(height) - extents.height()) / 2.0 - extents.y_bearing();

        // テキストを白色で描画
        cairo_context.set_source_rgb(1.0, 1.0, 1.0); // 白色
        cairo_context.move_to(x, y);
        cairo_context.show_text(text).context("テキスト描画に失敗")?;
    } // cairo_contextがここでドロップされる

    // Cairoサーフェスのデータを取得
    cairo_surface.flush();
    let cairo_data = cairo_surface.data()
        .context("Cairoデータの取得に失敗")?;

    // 一時ファイルを作成（共有メモリ用）
    let file = tempfile::tempfile()
        .context("一時ファイルの作成に失敗")?;

    // ファイルサイズを設定
    nix::unistd::ftruncate(&file, size as i64)
        .context("ファイルサイズの設定に失敗")?;

    // メモリマップ
    let mut mmap = unsafe {
        memmap2::MmapMut::map_mut(&file)
            .context("メモリマップに失敗")?
    };

    // CairoのデータをWaylandバッファにコピー
    mmap.copy_from_slice(&cairo_data);

    // 共有メモリプールを作成
    let pool = shm.create_pool(
        file.as_fd(),
        size,
        qh,
        (),
    );

    // バッファを作成
    let buffer = pool.create_buffer(
        0, // offset
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

// アプリケーション状態（イベントハンドラ用）
struct AppState {
    configured: bool,
}

impl AppState {
    fn new() -> Self {
        Self { configured: false }
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
    ) {
        // レジストリイベントは無視
    }
}

impl Dispatch<wl_compositor::WlCompositor, ()> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_compositor::WlCompositor,
        _event: wl_compositor::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // コンポジタイベントは無視
    }
}

impl Dispatch<wl_surface::WlSurface, ()> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_surface::WlSurface,
        _event: wl_surface::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // サーフェスイベントは無視
    }
}

impl Dispatch<wl_shm::WlShm, ()> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_shm::WlShm,
        _event: wl_shm::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // SHMイベントは無視
    }
}

impl Dispatch<wl_shm_pool::WlShmPool, ()> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_shm_pool::WlShmPool,
        _event: wl_shm_pool::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // SHMプールイベントは無視
    }
}

impl Dispatch<wl_buffer::WlBuffer, ()> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_buffer::WlBuffer,
        _event: wl_buffer::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // バッファイベントは無視
    }
}

impl Dispatch<wl_output::WlOutput, ()> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_output::WlOutput,
        _event: wl_output::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // 出力イベントは無視
    }
}

impl Dispatch<ZwlrLayerShellV1, ()> for AppState {
    fn event(
        _state: &mut Self,
        _proxy: &ZwlrLayerShellV1,
        _event: zwlr_layer_shell_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // Layer Shellイベントは無視
    }
}

impl Dispatch<ZwlrLayerSurfaceV1, ()> for AppState {
    fn event(
        state: &mut Self,
        _proxy: &ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_layer_surface_v1::Event::Configure { serial, .. } => {
                _proxy.ack_configure(serial);
                state.configured = true;
            }
            zwlr_layer_surface_v1::Event::Closed => {
                // サーフェスがクローズされた
            }
            _ => {}
        }
    }
}
