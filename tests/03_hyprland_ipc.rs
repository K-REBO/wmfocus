// Hyprland IPC動作テスト（最小版）
// このテストは、Hyprland IPCの基本的な動作を確認します。
//
// 実行方法:
//   cargo run --bin test_hyprland_ipc --features hyprland
//
// 前提条件:
//   - Hyprlandが起動していること

use anyhow::{Context, Result};

fn main() -> Result<()> {
    println!("=== Hyprland IPC 動作テスト ===\n");

    // テスト1: 基本的な接続テスト
    println!("【テスト1】Hyprland接続テスト");
    println!("{}", "-".repeat(50));
    test_connection()?;
    println!();

    // テスト2: ウィンドウリストの取得
    println!("【テスト2】ウィンドウリスト取得");
    println!("{}", "-".repeat(50));
    test_get_windows()?;
    println!();

    // テスト3: モニター情報の取得
    println!("【テスト3】モニター情報取得");
    println!("{}", "-".repeat(50));
    test_get_monitors()?;
    println!();

    // テスト4: 可視ウィンドウのフィルタリング
    println!("【テスト4】可視ウィンドウのフィルタリング");
    println!("{}", "-".repeat(50));
    test_filter_visible_windows()?;
    println!();

    // テスト5: フォーカス制御（可視タイル間）
    println!("【テスト5】フォーカス制御（可視タイル間）");
    println!("{}", "-".repeat(50));
    test_focus()?;
    println!();

    println!("=== 全テスト完了 ===");
    Ok(())
}

/// テスト1: 基本的な接続テスト
fn test_connection() -> Result<()> {
    use hyprland::data::Version;
    use hyprland::prelude::*;

    let version = Version::get().context("Hyprlandバージョンの取得に失敗")?;
    println!("✓ Hyprland接続成功");
    println!("  ブランチ: {}", version.branch);
    println!("  コミット: {}", version.commit);
    println!("  タグ: {}", version.tag);

    Ok(())
}

/// テスト2: ウィンドウリストの取得
fn test_get_windows() -> Result<()> {
    use hyprland::data::Clients;
    use hyprland::prelude::*;

    let clients = Clients::get().context("ウィンドウリストの取得に失敗")?;
    let client_vec = clients.to_vec();

    println!("✓ ウィンドウリスト取得成功");
    println!("  全ウィンドウ数: {}", client_vec.len());

    for (i, client) in client_vec.iter().enumerate() {
        println!("\nウィンドウ {}: {}", i + 1, client.title);
        println!("  アドレス: {}", client.address);
        println!("  クラス: {}", client.class);
        println!("  ワークスペース: {} (ID: {})",
            client.workspace.name, client.workspace.id);
        println!("  モニター: {:?}", client.monitor);
        println!("  位置: ({}, {})", client.at.0, client.at.1);
        println!("  サイズ: {}x{}", client.size.0, client.size.1);
        println!("  フローティング: {}", client.floating);
        println!("  フルスクリーン: {:?}", client.fullscreen);

        // focusedフィールドの有無を確認
        // もし存在すればコメントアウトを外す
        // println!("  フォーカス中: {}", client.focused);
    }

    Ok(())
}

/// テスト3: モニター情報の取得
fn test_get_monitors() -> Result<()> {
    use hyprland::data::Monitors;
    use hyprland::prelude::*;

    let monitors = Monitors::get().context("モニター情報の取得に失敗")?;
    let monitor_vec = monitors.to_vec();

    println!("✓ モニター情報取得成功");
    println!("  モニター数: {}", monitor_vec.len());

    for (i, monitor) in monitor_vec.iter().enumerate() {
        println!("\nモニター {}: {}", i + 1, monitor.name);
        println!("  ID: {:?}", monitor.id);
        println!("  解像度: {}x{}", monitor.width, monitor.height);
        println!("  リフレッシュレート: {:.2} Hz", monitor.refresh_rate);
        println!("  スケール: {:.2}", monitor.scale);
        println!("  位置: ({}, {})", monitor.x, monitor.y);
        println!("  アクティブワークスペース: {} (ID: {})",
            monitor.active_workspace.name,
            monitor.active_workspace.id);
    }

    Ok(())
}

/// テスト4: 可視ウィンドウのフィルタリング
/// wmfocusの本来の目的：アクティブワークスペース（可視領域）内のタイルのみを対象にする
fn test_filter_visible_windows() -> Result<()> {
    use hyprland::data::{Clients, Monitors, Workspace};
    use hyprland::prelude::*;

    // アクティブワークスペースを取得
    let active_workspace = Workspace::get_active()
        .context("アクティブワークスペースの取得に失敗")?;

    println!("アクティブワークスペース:");
    println!("  ID: {}", active_workspace.id);
    println!("  名前: {}", active_workspace.name);
    println!("  ウィンドウ数: {}", active_workspace.windows);

    // 全ウィンドウを取得
    let all_clients = Clients::get().context("ウィンドウリストの取得に失敗")?;
    let all_clients_vec = all_clients.to_vec();

    // モニター情報を取得して可視ワークスペースIDを収集
    let monitors = Monitors::get().context("モニター情報の取得に失敗")?;
    let monitor_vec = monitors.to_vec();

    let visible_workspace_ids: Vec<i32> = monitor_vec
        .iter()
        .map(|m| m.active_workspace.id)
        .collect();

    println!("\n可視ワークスペースID: {:?}", visible_workspace_ids);

    // 可視ウィンドウのみをフィルタリング
    let visible_clients: Vec<_> = all_clients_vec
        .iter()
        .filter(|c| visible_workspace_ids.contains(&c.workspace.id))
        .collect();

    println!("\n全ウィンドウ数: {}", all_clients_vec.len());
    println!("可視ウィンドウ数（タイル数）: {}", visible_clients.len());

    println!("\n可視タイル一覧:");
    for (i, client) in visible_clients.iter().enumerate() {
        println!("  {}. [{}] {} ({}x{} at {},{}) - WS: {}",
            i + 1,
            client.class,
            client.title,
            client.size.0,
            client.size.1,
            client.at.0,
            client.at.1,
            client.workspace.name
        );
    }

    Ok(())
}

/// テスト5: フォーカス制御（可視タイル間）
/// wmfocusの本来の動作：可視タイル間でのフォーカス移動
fn test_focus() -> Result<()> {
    use hyprland::data::{Client, Clients, Monitors};
    use hyprland::dispatch::{Dispatch, DispatchType, WindowIdentifier};
    use hyprland::prelude::*;

    // アクティブウィンドウを取得
    let active = match Client::get_active() {
        Ok(client_opt) => {
            if let Some(client) = client_opt {
                println!("現在のアクティブタイル:");
                println!("  タイトル: {}", client.title);
                println!("  クラス: {}", client.class);
                println!("  アドレス: {}", client.address);
                println!("  ワークスペース: {}", client.workspace.name);
                Some(client)
            } else {
                println!("アクティブウィンドウなし");
                None
            }
        }
        Err(e) => {
            println!("アクティブウィンドウ取得エラー: {}", e);
            None
        }
    };

    // 全ウィンドウを取得
    let all_clients = Clients::get().context("ウィンドウリストの取得に失敗")?;
    let all_clients_vec = all_clients.to_vec();

    // 可視ワークスペースのウィンドウのみをフィルタリング
    let monitors = Monitors::get().context("モニター情報の取得に失敗")?;
    let monitor_vec = monitors.to_vec();

    let visible_workspace_ids: Vec<i32> = monitor_vec
        .iter()
        .map(|m| m.active_workspace.id)
        .collect();

    let visible_clients: Vec<_> = all_clients_vec
        .iter()
        .filter(|c| visible_workspace_ids.contains(&c.workspace.id))
        .collect();

    println!("\n可視タイル数: {}", visible_clients.len());

    if visible_clients.is_empty() {
        println!("⚠️  可視タイルが存在しないため、フォーカステストをスキップします");
        return Ok(());
    }

    if visible_clients.len() < 2 {
        println!("⚠️  可視タイルが1つしかないため、フォーカステストをスキップします");
        println!("   （wmfocusは複数タイルがある場合に有用です）");
        return Ok(());
    }

    // アクティブでない可視タイルを探す
    let target = if let Some(active_window) = &active {
        visible_clients.iter()
            .find(|c| c.address != active_window.address)
            .unwrap_or(&visible_clients[0])
    } else {
        &visible_clients[0]
    };

    println!("\nフォーカス対象タイル:");
    println!("  タイトル: {}", target.title);
    println!("  クラス: {}", target.class);
    println!("  アドレス: {}", target.address);
    println!("  位置: ({}, {})", target.at.0, target.at.1);

    // フォーカスを実行
    println!("\n可視タイル間でフォーカスを移動します...");
    Dispatch::call(DispatchType::FocusWindow(WindowIdentifier::Address(
        target.address.clone(),
    )))
    .context("フォーカスの変更に失敗")?;

    println!("✓ フォーカス変更成功");

    // 少し待って確認
    std::thread::sleep(std::time::Duration::from_millis(500));

    // 確認
    if let Ok(new_active_opt) = Client::get_active() {
        if let Some(new_active) = new_active_opt {
            if new_active.address == target.address {
                println!("✓ フォーカス確認: 正しくフォーカスされました");
                println!("  → 可視タイル間のフォーカス移動が正常に動作");
            } else {
                println!("⚠️  フォーカス確認: 予期しないウィンドウがフォーカスされています");
                println!("   期待: {}", target.address);
                println!("   実際: {}", new_active.address);
            }
        }
    }

    Ok(())
}
