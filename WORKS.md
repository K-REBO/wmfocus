# wmfocus Hyprland移植 - 実装計画書

## 📖 プロジェクト概要

### 目的
i3ウィンドウマネージャー用に開発されたwmfocusをHyprland（Waylandコンポジタ）用に移植する。

### wmfocusとは
キーボードショートカットで起動すると、各ウィンドウにヒントラベル（例: "s", "a", "d"）を表示し、対応するキーを押すことで即座にそのウィンドウにフォーカスできるツール。マウスや方向キー操作なしに高速なウィンドウ切り替えを実現する。

### 現在の状況
- **完成**: i3ウィンドウマネージャー版（X11ベース）
- **部分実装**: Sway版（Waylandベース、機能不完全）
- **目標**: Hyprland版（Wayland、完全機能実装）

## 🏗️ 現在のアーキテクチャ（i3版）

### コンポーネント構成

```
src/
├── main.rs           # メインロジック、イベントループ、描画制御
├── args.rs           # CLI引数パース、設定管理
├── utils.rs          # ユーティリティ関数（ヒント生成、描画、入力処理）
└── wm_i3.rs          # i3固有の実装（ウィンドウ取得、フォーカス制御）
```

### データフロー

```
起動
  ↓
[wm_i3::get_windows()] → DesktopWindow[]
  ↓
[main] ウィンドウごとにヒント生成 + X11オーバーレイ作成
  ↓
[main] X11でキーボード・マウスグラブ
  ↓
[イベントループ]
  ├─ Expose → Cairo描画
  ├─ KeyPress → ヒント照合
  │    ├─ 完全一致 → wm_i3::focus_window() → 終了
  │    ├─ 部分一致 → 描画更新（一致する可能性のあるヒントを強調）
  │    └─ 不一致 → 終了またはキー削除
  └─ Escape/ButtonPress → 終了
```

### 主要データ構造

```rust
pub struct DesktopWindow {
    id: i64,                    // WM内部ID（i3のcon_id）
    x_window_id: Option<i32>,   // X11ウィンドウID
    pos: (i32, i32),            // ウィンドウ位置
    size: (i32, i32),           // ウィンドウサイズ
    is_focused: bool,           // 現在フォーカスされているか
}

pub struct RenderWindow<'a> {
    desktop_window: &'a DesktopWindow,
    cairo_context: cairo::Context,      // Cairo描画コンテキスト
    draw_pos: (f64, f64),                // テキスト描画位置
    rect: (i32, i32, i32, i32),          // オーバーレイの矩形（x, y, w, h）
}
```

### 使用技術スタック（i3版）

| コンポーネント | 技術 | クレート |
|--------------|------|---------|
| ウィンドウ管理 | i3 IPC | `i3ipc` |
| ウィンドウシステム | X11/XCB | `x11rb` |
| 描画 | Cairo | `cairo-rs` (xcb feature) |
| 入力 | X11 Grab | `x11rb::protocol::xproto` |
| キーシンボル変換 | X11 keysym | `xkeysym` |

## 🎯 Hyprland版の目標アーキテクチャ

### 技術スタック置き換え

| コンポーネント | i3版 | Hyprland版 | 変更理由 |
|--------------|------|-----------|---------|
| ウィンドウ管理 | i3 IPC | Hyprland IPC | WM変更 |
| ウィンドウシステム | X11/XCB | Wayland | プロトコル変更 |
| オーバーレイ作成 | XCB create_window | wlr-layer-shell-v1 | Waylandでのオーバーレイプロトコル |
| 描画 | Cairo (XCB) | Cairo (Wayland) | バックエンド変更 |
| 入力グラブ | X11 grab_keyboard | Layer Shell exclusive keyboard | Waylandの制約 |
| キーシンボル変換 | X11 keysym | xkbcommon | Wayland標準 |

### 新しいコンポーネント構成

```
src/
├── main.rs              # メインロジック（Wayland版に書き換え）
├── args.rs              # CLI引数パース（変更なし）
├── utils.rs             # 共通ユーティリティ（一部変更）
├── wm_hyprland.rs       # Hyprland IPC実装（新規作成）
├── wayland_surface.rs   # Waylandサーフェス管理（新規作成）
└── render.rs            # Cairo描画ロジック（新規作成、utilsから分離）
```

### 新しい依存関係

```toml
[dependencies]
# Hyprland IPC
hyprland = "0.4.0-beta.3"

# Wayland クライアント
wayland-client = "0.31"
wayland-protocols = "0.31"
wayland-protocols-wlr = "0.2"

# Cairo描画（バックエンド変更）
cairo-rs = { version = "0.20", features = ["use_glib"] }

# キーボード処理
xkbcommon = "0.7"

# 既存の依存関係
anyhow = "1"
clap = { version = "4", features = ["derive", "cargo", "wrap_help", "deprecated"] }
css-color-parser = "0.1"
font-loader = "0.11"
itertools = "0.13"
log = "0.4"
pretty_env_logger = "0.5"
regex = "1.10"
```

## 🧪 テスト駆動開発戦略

### 開発方針
技術的難易度が高いコンポーネントから順に、独立した動作テストを作成して動作確認してから本実装に組み込む。

### テスト優先順位（難易度順）

#### 1. **最高難度: Waylandレンダリング** (`tests/01_wayland_layer_shell.rs`)

**検証内容:**
- Waylandコンポジタへの接続
- wlr-layer-shell-v1プロトコルでのサーフェス作成
- 複数のオーバーレイサーフェス配置
- Cairoでの描画とバッファ管理

**成功基準:**
- Hyprland上で指定座標に透明な矩形ウィンドウが表示される
- Cairoで描画したテキストが正しく表示される
- 複数のサーフェスが同時に表示される

**技術的課題:**
- Layer Shellの正しい設定（anchor, exclusive_zone, layer）
- Waylandバッファプール管理
- Cairo ImageSurface → Wayland shared memory buffer変換
- ARGB8888フォーマットでの透明度

#### 2. **高難度: Wayland入力処理** (`tests/02_wayland_keyboard.rs`)

**検証内容:**
- Wayland Seatプロトコルでのキーボード取得
- Layer Shellの`keyboard-interactivity = exclusive`動作確認
- xkbcommonでのキーコード→キーシンボル変換
- キーボードイベントループ

**成功基準:**
- キー入力を受け取れる
- Escapeキーで終了できる
- 複数キーの組み合わせを認識できる
- 他のアプリケーションがキー入力を受け取らない（exclusive）

**技術的課題:**
- `keyboard-interactivity`の正しい設定
- xkbcommonキーマップの取得とパース
- キーリピートの処理
- モディファイアキー（Ctrl, Shiftなど）の処理

#### 3. **中難度: Hyprland IPC** (`tests/03_hyprland_ipc.rs`)

**検証内容:**
- `hyprland`クレートでウィンドウリスト取得
- フォーカス制御
- ウィンドウスワップ

**成功基準:**
- 可視ワークスペースのウィンドウ一覧が取得できる
- ウィンドウのクラス名、タイトル、座標、サイズが取得できる
- `dispatch focuswindow`でウィンドウフォーカスできる
- スワップ機能が動作する

**技術的課題:**
- 複数モニター環境での座標計算
- フローティングウィンドウとタイリングウィンドウの区別
- フルスクリーンウィンドウの扱い

## 📋 実装ステップ

### Phase 0: 環境準備
- [x] プロジェクト構造の理解
- [ ] Hyprland開発環境のセットアップ
- [ ] 必要な依存パッケージのインストール
- [ ] `tests/`ディレクトリ作成

### Phase 1: テスト作成と検証（技術検証フェーズ）

#### ステップ1-1: Hyprland IPCテスト
```bash
# 実装: tests/03_hyprland_ipc.rs
# 実行: cargo run --bin test_hyprland_ipc
```

**実装内容:**
```rust
// 1. ウィンドウリスト取得のテスト
// 2. フォーカス変更のテスト
// 3. 座標・サイズ取得の精度確認
```

#### ステップ1-2: Waylandレンダリングテスト
```bash
# 実装: tests/01_wayland_layer_shell.rs
# 実行: cargo run --bin test_wayland_render
```

**実装内容:**
```rust
// 1. 基本的なLayer Shellサーフェス作成
// 2. 単一の透明矩形を表示
// 3. Cairoテキスト描画
// 4. 複数サーフェスの同時表示
```

#### ステップ1-3: Wayland入力テスト
```bash
# 実装: tests/02_wayland_keyboard.rs
# 実行: cargo run --bin test_wayland_input
```

**実装内容:**
```rust
// 1. キーボード入力受け取り
// 2. xkbcommon統合
// 3. exclusive keyboard interactivity確認
```

### Phase 2: コアモジュール実装

#### ステップ2-1: `wm_hyprland.rs`実装
テスト結果をベースに実装:

```rust
pub fn get_windows() -> Result<Vec<DesktopWindow>> {
    // hyprland::data::Clients::get()を使用
    // 可視ワークスペースのフィルタリング
    // DesktopWindow構造体への変換
}

pub fn focus_window(window: &DesktopWindow) -> Result<()> {
    // hyprland::dispatch::Dispatch::call()を使用
    // DispatchType::FocusWindow
}

pub fn swap_windows(window1: &DesktopWindow, window2: &DesktopWindow) -> Result<()> {
    // hyprland::dispatch::Dispatch::call()を使用
    // DispatchType::SwapWindow
}
```

#### ステップ2-2: `wayland_surface.rs`実装
テスト結果をベースに実装:

```rust
pub struct WaylandContext {
    display: Display,
    queue: EventQueue,
    layer_shell: zwlr_layer_shell_v1,
    compositor: wl_compositor,
    // ... その他必要なグローバル
}

pub struct LayerSurface {
    surface: wl_surface,
    layer_surface: zwlr_layer_surface_v1,
    buffer: ShmBuffer,
    cairo_surface: cairo::ImageSurface,
}

impl LayerSurface {
    pub fn new(context: &WaylandContext, rect: (i32, i32, i32, i32)) -> Result<Self>;
    pub fn get_cairo_context(&self) -> &cairo::Context;
    pub fn commit(&self) -> Result<()>;
}
```

#### ステップ2-3: `render.rs`実装
`utils.rs`から描画ロジックを分離:

```rust
pub fn draw_hint_text(
    cairo_ctx: &cairo::Context,
    hint: &str,
    pressed_keys: &str,
    draw_pos: (f64, f64),
    config: &AppConfig,
    is_focused: bool,
) -> Result<()> {
    // 既存のutils::draw_hint_textから移植
    // Waylandサーフェス用に調整
}
```

### Phase 3: メインロジック統合

#### ステップ3-1: `main.rs`のWayland化
1. X11/XCB関連コードを削除
2. Waylandコンテキスト初期化
3. DesktopWindow取得（`wm_hyprland::get_windows()`）
4. 各ウィンドウ用LayerSurface作成
5. Waylandイベントループ実装

**イベントループ構造:**
```rust
loop {
    // Waylandイベント処理
    queue.dispatch(&mut state, |event, object, state| {
        match event {
            // キーボードイベント
            KeyboardEvent::Key { key, state: KeyState::Pressed, .. } => {
                // ヒント照合ロジック
                // フォーカス制御
            }

            // サーフェス再描画
            SurfaceEvent::Configure { .. } => {
                // Cairo描画
                // バッファコミット
            }

            _ => {}
        }
    })?;

    if should_exit {
        break;
    }
}
```

### Phase 4: テストと調整

- [ ] 単一モニター環境でテスト
- [ ] 複数モニター環境でテスト
- [ ] HiDPI環境でテスト
- [ ] フローティング/タイリング混在環境でテスト
- [ ] パフォーマンス測定とボトルネック解消
- [ ] メモリリークチェック

### Phase 5: ドキュメント整備

- [ ] README.md更新
- [ ] コード内ドキュメント追加
- [ ] ビルド手順更新
- [ ] トラブルシューティングガイド作成

## 🔧 開発環境セットアップ

### 必須要件
- Hyprland 0.35.0以降
- Rust 1.70以降
- Wayland開発ライブラリ

### Arch Linuxでのセットアップ
```bash
# Hyprlandインストール
sudo pacman -S hyprland

# 開発ライブラリ
sudo pacman -S wayland wayland-protocols cairo pkgconf

# Rustツールチェーン
rustup update stable
```

### ビルド
```bash
# 依存関係追加後
cargo build --release

# テスト実行
cargo run --bin test_hyprland_ipc
cargo run --bin test_wayland_render
cargo run --bin test_wayland_input

# 本体実行
cargo run --release
```

## 🐛 技術的課題と解決策

### 課題1: Waylandキーボードの排他的グラブ

**問題:**
WaylandにはX11の`GrabKeyboard`に相当する強制的なグラブ機能がない。

**解決策:**
- Layer Shellの`keyboard-interactivity`を`exclusive`に設定
- レイヤーを`Overlay`に設定（最前面）
- これにより事実上の排他的入力が可能

**設定例:**
```rust
layer_surface.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
layer_surface.set_layer(Layer::Overlay);
```

### 課題2: 複数サーフェスの座標計算

**問題:**
HyprlandはWayland座標（グローバル座標）を使用。スケーリングやモニター配置を考慮する必要がある。

**解決策:**
1. Hyprland IPCから正確なウィンドウ座標を取得（`at`フィールド）
2. モニター情報を取得してスケーリング係数を適用
3. Layer Surfaceの`set_size`と`set_exclusive_zone(0)`で正確な配置

**実装例:**
```rust
let monitors = hyprland::data::Monitors::get()?;
let monitor = monitors.iter()
    .find(|m| m.id == window.monitor_id)?;

let scale = monitor.scale;
let adjusted_x = (window.at.0 as f64 * scale) as i32;
let adjusted_y = (window.at.1 as f64 * scale) as i32;
```

### 課題3: Cairoバッファ管理

**問題:**
CairoのImageSurfaceをWaylandのshared memory bufferに変換する必要がある。

**解決策:**
1. `cairo::ImageSurface::create(Format::ARgb32, w, h)`で作成
2. データポインタを取得して`memfd`または`shm_open`で共有メモリ作成
3. `wl_shm_pool`経由で`wl_buffer`作成
4. Cairo描画後、`wl_surface::attach`と`commit`

**参考実装:**
- `wayland-client`の`MemPool`を使用
- または`wayland-rs`のサンプルコードを参考

### 課題4: ヒント重複の回避

**問題:**
既存コードのオーバーレイ重複検出がX11座標ベース。

**解決策:**
- `utils::find_overlaps()`はそのまま使用可能
- Wayland座標に変換してから重複チェック
- Layer Surfaceの位置は`set_margin()`で微調整

## 📚 参考資料

### Hyprland
- [Hyprland Wiki - IPC](https://wiki.hypr.land/IPC/)
- [hyprland-rs Documentation](https://docs.rs/hyprland/)
- [hyprland-rs Examples](https://github.com/hyprland-community/hyprland-rs/tree/master/examples)

### Wayland
- [Wayland Protocol Documentation](https://wayland.app/protocols/)
- [wlr-layer-shell Protocol](https://wayland.app/protocols/wlr-layer-shell-unstable-v1)
- [wayland-rs Book](https://smithay.github.io/wayland-rs/)
- [Wayland by Example](https://bugaevc.gitbooks.io/writing-wayland-clients/content/)

### Cairo
- [cairo-rs Documentation](https://docs.rs/cairo-rs/)
- [Cairo Graphics Tutorial](https://www.cairographics.org/tutorial/)

### 参考実装
- [waybar](https://github.com/Alexays/Waybar) - Layer Shell使用例
- [wlogout](https://github.com/ArtsyMacaw/wlogout) - オーバーレイUI実装
- [rofi-wayland](https://github.com/lbonn/rofi) - Waylandランチャー

## 📝 実装メモ

### データ構造の変更

**DesktopWindow構造体の更新:**
```rust
pub struct DesktopWindow {
    address: String,        // Hyprlandのウィンドウアドレス（0x...形式）
    pos: (i32, i32),        // Wayland座標
    size: (i32, i32),       // ピクセルサイズ
    monitor_id: i64,        // モニターID
    workspace_id: i64,      // ワークスペースID
    is_focused: bool,
    title: String,          // デバッグ用
    class: String,          // デバッグ用
}
```

### イベントループの状態管理

```rust
struct AppState {
    layer_surfaces: HashMap<String, LayerSurface>,  // hint -> surface
    desktop_windows: HashMap<String, DesktopWindow>, // hint -> window
    pressed_keys: String,
    should_exit: bool,
    config: AppConfig,
}
```

## ✅ チェックリスト

### Phase 0: 準備
- [x] Hyprlandが動作している
- [x] 開発ライブラリがインストールされている
- [x] `tests/`ディレクトリを作成

### Phase 1: テスト
- [x] `tests/03_hyprland_ipc.rs`実装・動作確認
- [x] `tests/01_wayland_layer_shell.rs`実装・動作確認
- [x] `tests/02_wayland_keyboard.rs`実装・動作確認
- [x] `tests/04_draw_tile_boxes.rs`実装・動作確認
- [x] `tests/05_draw_all_boxes.rs`実装・動作確認

### Phase 2: コアモジュール
- [x] `src/wm_hyprland.rs`実装
- [x] `src/wayland_render.rs`実装（wayland_surfaceとrenderを統合）

### Phase 3: 統合
- [x] `src/main.rs`のWayland化
- [x] 基本動作確認（ウィンドウにラベル表示）
- [x] フォーカス制御動作確認

### Phase 4: テスト・調整
- [x] 各種環境でテスト
- [x] バグ修正
- [x] パフォーマンス最適化（リリースビルド）
- [x] 画面サイズ動的取得
- [x] ヒント表示改善（角丸矩形、マージン調整）

### Phase 5: リリース準備
- [x] ドキュメント更新（README.md）
- [x] `Cargo.toml`更新
- [ ] AUR/パッケージング準備

---

## 🎉 実装完了

wmfocusのHyprland移植が完了しました！

### 実装された機能
- ✅ Hyprland IPC統合（hyprland-rs 0.4.0-beta.3）
- ✅ Wayland Layer Shell によるオーバーレイ表示
- ✅ Cairo による高品質な文字描画（角丸矩形、マージン調整）
- ✅ xkbcommon によるキーボード入力処理
- ✅ 同一ワークスペース内の可視タイルのみをフォーカス対象とする正確なフィルタリング
- ✅ 動的な画面サイズ検出（マルチモニター対応）
- ✅ リリースビルド対応（3.3MB）

### ビルド・実行方法
```bash
# デバッグビルド
cargo build --features hyprland

# リリースビルド
cargo build --release --features hyprland

# 実行
cargo run --features hyprland --bin wmfocus
# または
./target/release/wmfocus
```

### 今後の拡張案
- [ ] AUR パッケージング
- [ ] 複数モニターでの詳細なテスト
- [ ] カスタムフォント対応の復元（Cairoフォント選択機能）
- [ ] アニメーション効果の追加
