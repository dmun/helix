use helix_core::Position;
use helix_view::{
    graphics::{CursorKind, Rect},
    Editor,
};
use tui::buffer::Buffer;

use crate::compositor::{Component, Context, Event, EventResult};

/// Contains a component placed in the center of the parent component
pub struct Overlay<T> {
    /// Child component
    pub content: T,
    /// Function to compute the size and position of the child component
    pub calc_child_size: Box<dyn Fn(Rect) -> Rect>,
}

/// Contains a component docked to the bottom of the screen.
pub struct BottomDock<T> {
    /// Child component.
    pub content: T,
}

/// Dock a component to the bottom 40% of the screen.
pub fn bottom_docked<T>(content: T) -> BottomDock<T> {
    BottomDock { content }
}

impl<T> BottomDock<T> {
    fn child_area(area: Rect) -> Rect {
        // Leave at least one document row and its statusline visible. On very
        // small terminals the dock shrinks below its preferred minimum.
        let max_height = area.height.saturating_sub(2);
        let preferred_height = ((u32::from(area.height) * 40) / 100) as u16;
        let height = preferred_height.max(6).min(max_height);

        area.clip_top(area.height.saturating_sub(height))
    }

    fn relocate_statusline(editor_area: Rect, child_area: Rect, frame: &mut Buffer) {
        if editor_area.area() == 0 || child_area.y <= editor_area.y {
            return;
        }

        // Copy the already-rendered bottom view statusline above the dock before
        // the dock covers it. The editor retains its full viewport, so opening
        // the dock does not invoke resize/scrolloff and change the document's
        // view offset.
        let source_y = editor_area.bottom() - 1;
        let target_y = child_area.y - 1;
        for x in editor_area.left()..editor_area.right() {
            if let Some(cell) = frame.get(x, source_y).cloned() {
                if let Some(target) = frame.get_mut(x, target_y) {
                    *target = cell;
                }
            }
        }
    }
}

/// Surrounds the component with a margin of 5% on each side, and an additional 2 rows at the bottom
pub fn overlaid<T>(content: T) -> Overlay<T> {
    Overlay {
        content,
        calc_child_size: Box::new(|rect: Rect| clip_rect_relative(rect.clip_bottom(2), 90, 90)),
    }
}

fn clip_rect_relative(rect: Rect, percent_horizontal: u8, percent_vertical: u8) -> Rect {
    fn mul_and_cast(size: u16, factor: u8) -> u16 {
        ((size as u32) * (factor as u32) / 100).try_into().unwrap()
    }

    let inner_w = mul_and_cast(rect.width, percent_horizontal);
    let inner_h = mul_and_cast(rect.height, percent_vertical);

    let offset_x = rect.width.saturating_sub(inner_w) / 2;
    let offset_y = rect.height.saturating_sub(inner_h) / 2;

    Rect {
        x: rect.x + offset_x,
        y: rect.y + offset_y,
        width: inner_w,
        height: inner_h,
    }
}

impl<T: Component + 'static> Component for Overlay<T> {
    fn render(&mut self, area: Rect, frame: &mut Buffer, ctx: &mut Context) {
        let dimensions = (self.calc_child_size)(area);
        self.content.render(dimensions, frame, ctx)
    }

    fn required_size(&mut self, (width, height): (u16, u16)) -> Option<(u16, u16)> {
        let area = Rect {
            x: 0,
            y: 0,
            width,
            height,
        };
        let dimensions = (self.calc_child_size)(area);
        let viewport = (dimensions.width, dimensions.height);
        let _ = self.content.required_size(viewport)?;
        Some((width, height))
    }

    fn handle_event(&mut self, event: &Event, ctx: &mut Context) -> EventResult {
        self.content.handle_event(event, ctx)
    }

    fn cursor(&self, area: Rect, ctx: &Editor) -> (Option<Position>, CursorKind) {
        let dimensions = (self.calc_child_size)(area);
        self.content.cursor(dimensions, ctx)
    }

    fn id(&self) -> Option<&'static str> {
        self.content.id()
    }
}

impl<T: Component + 'static> Component for BottomDock<T> {
    fn render(&mut self, area: Rect, frame: &mut Buffer, ctx: &mut Context) {
        let child_area = Self::child_area(area);
        if child_area.area() == 0 {
            return;
        }
        Self::relocate_statusline(ctx.editor.tree.area(), child_area, frame);
        self.content
            .required_size((child_area.width, child_area.height));
        self.content.render(child_area, frame, ctx)
    }

    fn required_size(&mut self, (width, height): (u16, u16)) -> Option<(u16, u16)> {
        let area = Self::child_area(Rect::new(0, 0, width, height));
        let _ = self.content.required_size((area.width, area.height))?;
        Some((width, height))
    }

    fn handle_event(&mut self, event: &Event, ctx: &mut Context) -> EventResult {
        self.content.handle_event(event, ctx)
    }

    fn cursor(&self, area: Rect, ctx: &Editor) -> (Option<Position>, CursorKind) {
        let area = Self::child_area(area);
        if area.area() == 0 {
            (None, CursorKind::Hidden)
        } else {
            self.content.cursor(area, ctx)
        }
    }

    fn id(&self) -> Option<&'static str> {
        self.content.id()
    }

    fn name(&self) -> Option<&str> {
        self.content.name()
    }
}

#[cfg(test)]
mod tests {
    use super::BottomDock;
    use helix_view::graphics::Rect;
    use tui::buffer::Buffer;

    #[test]
    fn bottom_dock_is_placed_below_the_relocated_statusline() {
        let viewport = Rect::new(0, 0, 100, 40);

        let dock = BottomDock::<()>::child_area(viewport);

        assert_eq!(dock, Rect::new(0, 24, 100, 16));
        assert_eq!(dock.y - 1, 23);
    }

    #[test]
    fn bottom_dock_preserves_two_editor_rows_on_small_terminals() {
        let viewport = Rect::new(4, 2, 20, 7);

        let dock = BottomDock::<()>::child_area(viewport);

        assert_eq!(dock, Rect::new(4, 4, 20, 5));
        assert_eq!(dock.y - 1, 3);
    }

    #[test]
    fn bottom_dock_hides_when_no_editor_space_is_available() {
        let viewport = Rect::new(0, 0, 20, 2);

        assert_eq!(BottomDock::<()>::child_area(viewport).area(), 0);
    }

    #[test]
    fn bottom_dock_copies_the_statusline_without_resizing_the_editor() {
        let viewport = Rect::new(0, 0, 10, 10);
        let dock = BottomDock::<()>::child_area(viewport);
        let editor_area = viewport.clip_bottom(1);
        let mut frame = Buffer::empty(viewport);
        frame.set_stringn(0, 8, "statusline", 10, Default::default());

        BottomDock::<()>::relocate_statusline(editor_area, dock, &mut frame);

        let copied: String = (0..10)
            .map(|x| frame.get(x, dock.y - 1).unwrap().symbol.as_str())
            .collect();
        assert_eq!(copied, "statusline");
    }
}
