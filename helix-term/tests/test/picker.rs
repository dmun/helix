use super::*;
use std::cell::Cell;

fn view_offset(app: &helix_term::application::Application) -> helix_view::view::ViewPosition {
    let (view, doc) = helix_view::current_ref!(app.editor);
    doc.view_offset(view.id)
}

fn assert_picker_open(app: &helix_term::application::Application) {
    // The picker covers the bottom of the editor without resizing its tree, so
    // opening it cannot invoke scrolloff or change the document view offset.
    assert_eq!(app.editor.tree.area().height, 149);
}

fn assert_picker_closed(app: &helix_term::application::Application) {
    // Closing the picker restores the normal command-line row.
    assert_eq!(app.editor.tree.area().height, 149);
}

#[tokio::test(flavor = "multi_thread")]
async fn picker_docks_without_resizing_or_scrolling_the_document() -> anyhow::Result<()> {
    let input = (0..300)
        .map(|line| format!("line {line}\n"))
        .collect::<String>();
    let mut app = AppBuilder::new()
        .with_input_text(format!("#[|]#{input}"))
        .build()?;
    let offset_before_picker = Cell::new(None);
    let capture_offset = |app: &helix_term::application::Application| {
        offset_before_picker.set(Some(view_offset(app)));
    };
    let assert_offset_unchanged = |app: &helix_term::application::Application| {
        assert_eq!(offset_before_picker.get(), Some(view_offset(app)));
    };

    test_key_sequences(
        &mut app,
        vec![
            (Some("G"), Some(&capture_offset as _)),
            (
                Some("<space>b"),
                Some(&|app| {
                    assert_picker_open(app);
                    assert_offset_unchanged(app);
                }),
            ),
            (
                Some("<C-c>"),
                Some(&|app| {
                    assert_picker_closed(app);
                    assert_offset_unchanged(app);
                }),
            ),
        ],
        false,
    )
    .await
}
