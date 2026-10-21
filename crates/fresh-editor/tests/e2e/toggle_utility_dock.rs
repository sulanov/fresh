//! Tests for `toggle_utility_dock` and `focus_utility_dock`.

use crate::common::harness::EditorTestHarness;
use fresh::input::keybindings::{Action, KeyContext};
use fresh::model::event::LeafId;
use fresh::view::split::SplitRole;
use std::fs;

struct DockFixture {
    harness: EditorTestHarness,
    _temp: tempfile::TempDir,
    editor_leaf: LeafId,
    dock_leaf: LeafId,
    _file_buffer: fresh::model::event::BufferId,
    terminal_buffer: fresh::model::event::BufferId,
}

fn setup_editor_with_dock_terminal() -> DockFixture {
    let temp = tempfile::TempDir::new().unwrap();
    let file = temp.path().join("main.txt");
    fs::write(&file, "hello world\n").unwrap();

    let mut harness = EditorTestHarness::new(120, 40).unwrap();
    harness.open_file(&file).unwrap();

    let file_buffer = harness.editor().active_buffer();
    let editor_leaf = harness.editor().split_manager_for_tests().active_split();

    harness
        .editor_mut()
        .dispatch_action_for_tests(Action::OpenTerminalInDock);

    let (dock_leaf, terminal_buffer) = {
        let sm = harness.editor().split_manager_for_tests();
        let dock_leaf = sm
            .find_leaf_by_role(SplitRole::UtilityDock)
            .expect("OpenTerminalInDock must create a dock leaf");
        let terminal_buffer = sm
            .get_buffer_id(dock_leaf.into())
            .expect("dock leaf must hold the terminal buffer");
        (dock_leaf, terminal_buffer)
    };

    DockFixture {
        harness,
        _temp: temp,
        editor_leaf,
        dock_leaf,
        _file_buffer: file_buffer,
        terminal_buffer,
    }
}

fn dock_leaf_tabs(
    harness: &EditorTestHarness,
    leaf: LeafId,
) -> Vec<fresh::model::event::BufferId> {
    harness
        .editor()
        .split_view_state_for_tests(leaf)
        .map(|vs| vs.buffer_tab_ids_vec())
        .unwrap_or_default()
}

#[test]
fn test_toggle_utility_dock_when_no_dock_shows_status_message() {
    let temp = tempfile::TempDir::new().unwrap();
    let file = temp.path().join("main.txt");
    fs::write(&file, "hello\n").unwrap();

    let mut harness = EditorTestHarness::new(120, 40).unwrap();
    harness.open_file(&file).unwrap();

    assert!(harness
        .editor()
        .split_manager_for_tests()
        .find_leaf_by_role(SplitRole::UtilityDock)
        .is_none());

    harness
        .editor_mut()
        .dispatch_action_for_tests(Action::ToggleUtilityDock);

    assert!(harness
        .editor()
        .split_manager_for_tests()
        .find_leaf_by_role(SplitRole::UtilityDock)
        .is_none());

    let status = harness.editor().active_window().status_message.clone();
    assert_eq!(
        status.as_deref(),
        Some("No Utility Dock open — invoke a dock-aware utility (Diagnostics, Search/Replace, …)")
    );
}

#[test]
fn test_toggle_utility_dock_hides_and_restores_dock() {
    let mut fx = setup_editor_with_dock_terminal();

    // Precondition: dock is open and focused
    assert_eq!(
        fx.harness
            .editor()
            .split_manager_for_tests()
            .find_leaf_by_role(SplitRole::UtilityDock),
        Some(fx.dock_leaf)
    );
    assert_eq!(
        fx.harness.editor().split_manager_for_tests().active_split(),
        fx.dock_leaf
    );
    assert_eq!(
        fx.harness.editor().active_window().key_context,
        KeyContext::Terminal
    );

    // Act 1: Toggle utility dock -> hides the dock
    fx.harness
        .editor_mut()
        .dispatch_action_for_tests(Action::ToggleUtilityDock);

    // Dock leaf is no longer in the split tree
    assert!(fx
        .harness
        .editor()
        .split_manager_for_tests()
        .find_leaf_by_role(SplitRole::UtilityDock)
        .is_none());

    // Focus returned to the editor leaf
    assert_eq!(
        fx.harness.editor().split_manager_for_tests().active_split(),
        fx.editor_leaf
    );
    assert_eq!(
        fx.harness.editor().active_window().key_context,
        KeyContext::Normal
    );

    // Terminal buffer is still alive
    assert!(fx
        .harness
        .editor()
        .active_window()
        .buffers
        .contains_key(&fx.terminal_buffer));

    // Act 2: Toggle utility dock again -> restores the dock
    fx.harness
        .editor_mut()
        .dispatch_action_for_tests(Action::ToggleUtilityDock);

    let restored_dock = fx
        .harness
        .editor()
        .split_manager_for_tests()
        .find_leaf_by_role(SplitRole::UtilityDock)
        .expect("dock leaf must be restored");

    // Restored dock has the terminal buffer
    assert!(dock_leaf_tabs(&fx.harness, restored_dock).contains(&fx.terminal_buffer));
    assert_eq!(
        fx.harness.editor().split_manager_for_tests().active_split(),
        restored_dock
    );
    assert_eq!(
        fx.harness.editor().active_window().key_context,
        KeyContext::Terminal
    );

    // Act 3: Toggle utility dock once more -> hides it again
    fx.harness
        .editor_mut()
        .dispatch_action_for_tests(Action::ToggleUtilityDock);

    assert!(fx
        .harness
        .editor()
        .split_manager_for_tests()
        .find_leaf_by_role(SplitRole::UtilityDock)
        .is_none());
    assert_eq!(
        fx.harness.editor().split_manager_for_tests().active_split(),
        fx.editor_leaf
    );
}

#[test]
fn test_focus_utility_dock_switches_focus_without_hiding() {
    let mut fx = setup_editor_with_dock_terminal();

    // Initially in dock
    assert_eq!(
        fx.harness.editor().split_manager_for_tests().active_split(),
        fx.dock_leaf
    );

    // Focus utility dock -> moves focus to editor, dock stays open
    fx.harness
        .editor_mut()
        .dispatch_action_for_tests(Action::FocusUtilityDock);

    assert_eq!(
        fx.harness
            .editor()
            .split_manager_for_tests()
            .find_leaf_by_role(SplitRole::UtilityDock),
        Some(fx.dock_leaf),
        "dock must remain open after focus switch"
    );
    assert_eq!(
        fx.harness.editor().split_manager_for_tests().active_split(),
        fx.editor_leaf
    );
    assert_eq!(
        fx.harness.editor().active_window().key_context,
        KeyContext::Normal
    );

    // Focus utility dock again -> moves focus back to dock
    fx.harness
        .editor_mut()
        .dispatch_action_for_tests(Action::FocusUtilityDock);

    assert_eq!(
        fx.harness.editor().split_manager_for_tests().active_split(),
        fx.dock_leaf
    );
    assert_eq!(
        fx.harness.editor().active_window().key_context,
        KeyContext::Terminal
    );
}

#[test]
fn test_open_terminal_in_dock_while_hidden_restores_dock_and_attaches_tab() {
    let mut fx = setup_editor_with_dock_terminal();

    // Hide dock
    fx.harness
        .editor_mut()
        .dispatch_action_for_tests(Action::ToggleUtilityDock);
    assert!(fx
        .harness
        .editor()
        .split_manager_for_tests()
        .find_leaf_by_role(SplitRole::UtilityDock)
        .is_none());

    // Open terminal in dock while hidden
    fx.harness
        .editor_mut()
        .dispatch_action_for_tests(Action::OpenTerminalInDock);

    let dock_leaf = fx
        .harness
        .editor()
        .split_manager_for_tests()
        .find_leaf_by_role(SplitRole::UtilityDock)
        .expect("dock must be restored");

    let tabs = dock_leaf_tabs(&fx.harness, dock_leaf);
    // Should have both the original terminal buffer and the new terminal buffer
    assert!(
        tabs.contains(&fx.terminal_buffer),
        "original terminal should still be in tabs"
    );
    assert_eq!(tabs.len(), 2, "dock should have 2 terminal tabs");
}

#[test]
fn test_closing_last_dock_tab_clears_saved_dock() {
    let mut fx = setup_editor_with_dock_terminal();

    // Close the sole terminal tab in the dock
    let closed = fx
        .harness
        .editor_mut()
        .close_tab_in_split(fx.terminal_buffer, fx.dock_leaf);
    assert!(closed);

    // Dock is closed
    assert!(fx
        .harness
        .editor()
        .split_manager_for_tests()
        .find_leaf_by_role(SplitRole::UtilityDock)
        .is_none());

    // Attempt to toggle dock: because the only tab was closed, there is no dock to restore
    fx.harness
        .editor_mut()
        .dispatch_action_for_tests(Action::ToggleUtilityDock);

    assert!(fx
        .harness
        .editor()
        .split_manager_for_tests()
        .find_leaf_by_role(SplitRole::UtilityDock)
        .is_none());
    assert_eq!(
        fx.harness.editor().active_window().status_message.as_deref(),
        Some("No Utility Dock open — invoke a dock-aware utility (Diagnostics, Search/Replace, …)")
    );
}

#[test]
fn test_toggle_utility_dock_preserves_active_editor_leaf_with_multiple_splits() {
    let temp = tempfile::TempDir::new().unwrap();
    let file1 = temp.path().join("a.txt");
    let file2 = temp.path().join("b.txt");
    fs::write(&file1, "aaa\n").unwrap();
    fs::write(&file2, "bbb\n").unwrap();

    let mut harness = EditorTestHarness::new(120, 40).unwrap();
    harness.open_file(&file1).unwrap();
    let _leaf_a = harness.editor().split_manager_for_tests().active_split();

    // Split vertically -> creates leaf B
    harness.editor_mut().split_pane_vertical();
    let leaf_b = harness.editor().split_manager_for_tests().active_split();
    harness.open_file(&file2).unwrap();
    assert_eq!(
        harness.editor().split_manager_for_tests().active_split(),
        leaf_b
    );

    // Open terminal in dock
    harness
        .editor_mut()
        .dispatch_action_for_tests(Action::OpenTerminalInDock);
    let dock_leaf = harness
        .editor()
        .split_manager_for_tests()
        .find_leaf_by_role(SplitRole::UtilityDock)
        .expect("dock leaf must exist");
    assert_eq!(
        harness.editor().split_manager_for_tests().active_split(),
        dock_leaf
    );

    // Toggle utility dock -> dock hides, focus returns to leaf B (last focused editor split)
    harness
        .editor_mut()
        .dispatch_action_for_tests(Action::ToggleUtilityDock);
    assert!(harness
        .editor()
        .split_manager_for_tests()
        .find_leaf_by_role(SplitRole::UtilityDock)
        .is_none());
    assert_eq!(
        harness.editor().split_manager_for_tests().active_split(),
        leaf_b
    );

    // Toggle utility dock -> dock restored, focus in dock
    harness
        .editor_mut()
        .dispatch_action_for_tests(Action::ToggleUtilityDock);
    let restored_dock = harness
        .editor()
        .split_manager_for_tests()
        .find_leaf_by_role(SplitRole::UtilityDock)
        .expect("dock leaf must be restored");
    assert_eq!(
        harness.editor().split_manager_for_tests().active_split(),
        restored_dock
    );

    // Toggle utility dock -> dock hides, focus returns to leaf B
    harness
        .editor_mut()
        .dispatch_action_for_tests(Action::ToggleUtilityDock);
    assert_eq!(
        harness.editor().split_manager_for_tests().active_split(),
        leaf_b
    );
}
