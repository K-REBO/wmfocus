use anyhow::{Context, Result};
use hyprland::data::{Client, Clients, Monitors};
use hyprland::dispatch::{Dispatch as HyprDispatch, DispatchType, WindowIdentifier};
use hyprland::prelude::*;
use log::{debug, info, warn};

use crate::DesktopWindow;

/// Return a list of all visible windows on active workspaces.
pub fn get_windows() -> Result<Vec<DesktopWindow>> {
    // Get all clients
    let clients = Clients::get().context("Failed to get clients from Hyprland")?;
    let client_vec = clients.to_vec();

    // Get monitors to determine visible workspaces
    let monitors = Monitors::get().context("Failed to get monitors from Hyprland")?;
    let monitor_vec = monitors.to_vec();

    // Collect active workspace IDs from all monitors
    let visible_workspace_ids: Vec<i32> = monitor_vec
        .iter()
        .map(|m| m.active_workspace.id)
        .collect();

    debug!("Visible workspace IDs: {:?}", visible_workspace_ids);

    // Filter clients to only those on visible workspaces
    let visible_clients: Vec<&Client> = client_vec
        .iter()
        .filter(|c| visible_workspace_ids.contains(&c.workspace.id))
        .collect();

    debug!("Found {} visible windows", visible_clients.len());

    // Get the currently focused client
    let active_address = hyprland::data::Client::get_active()
        .ok()
        .flatten()
        .map(|c| c.address);

    // Convert to DesktopWindow
    let mut windows = Vec::new();
    for client in visible_clients {
        // Use the address as a unique ID (convert the string representation to a hash)
        let id = {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            client.address.to_string().hash(&mut hasher);
            hasher.finish() as i64
        };

        let window = DesktopWindow {
            id,
            x_window_id: None, // Wayland doesn't use X11 window IDs
            pos: (client.at.0 as i32, client.at.1 as i32),
            size: (client.size.0 as i32, client.size.1 as i32),
            is_focused: active_address.as_ref() == Some(&client.address),
        };
        debug!("Found window: {:?}", window);
        windows.push(window);
    }

    Ok(windows)
}

/// Focus a specific window by its ID.
pub fn focus_window(window: &DesktopWindow) -> Result<()> {
    // We need to find the client by position since we can't reconstruct the address from ID
    let clients = Clients::get().context("Failed to get clients")?;
    let client_vec = clients.to_vec();

    // Find the client that matches this window's position and size
    let target_client = client_vec
        .iter()
        .find(|c| {
            c.at.0 as i32 == window.pos.0
                && c.at.1 as i32 == window.pos.1
                && c.size.0 as i32 == window.size.0
                && c.size.1 as i32 == window.size.1
        })
        .context("Could not find matching client")?;

    info!("Focusing window at ({}, {})", window.pos.0, window.pos.1);

    HyprDispatch::call(DispatchType::FocusWindow(WindowIdentifier::Address(
        target_client.address.clone(),
    )))
    .context("Failed to focus window")?;

    Ok(())
}

/// Swap two windows.
#[allow(dead_code)]
pub fn swap_windows(active_window: &DesktopWindow, window: &DesktopWindow) -> Result<()> {
    // We need to find the clients by position
    let clients = Clients::get().context("Failed to get clients")?;
    let client_vec = clients.to_vec();

    let _active_client = client_vec
        .iter()
        .find(|c| {
            c.at.0 as i32 == active_window.pos.0
                && c.at.1 as i32 == active_window.pos.1
                && c.size.0 as i32 == active_window.size.0
                && c.size.1 as i32 == active_window.size.1
        })
        .context("Could not find active client")?;

    let target_client = client_vec
        .iter()
        .find(|c| {
            c.at.0 as i32 == window.pos.0
                && c.at.1 as i32 == window.pos.1
                && c.size.0 as i32 == window.size.0
                && c.size.1 as i32 == window.size.1
        })
        .context("Could not find target client")?;

    info!(
        "Swapping windows at ({}, {}) <-> ({}, {})",
        active_window.pos.0, active_window.pos.1, window.pos.0, window.pos.1
    );

    // TODO: Implement proper window swapping for Hyprland
    // Hyprland's SwapWindow uses Direction, not WindowIdentifier
    // For now, just focus the target window
    HyprDispatch::call(DispatchType::FocusWindow(WindowIdentifier::Address(
        target_client.address.clone(),
    )))
    .context("Failed to focus target window")?;

    warn!("Window swapping not fully implemented for Hyprland yet");

    Ok(())
}
