//! A multi-line, soft-wrapping text editor with externally supplied highlights.
//!
//! GPUI ships no text widget, so this is built directly on its text shaping and
//! `EntityInputHandler` (which also provides IME support). It is based on GPUI's
//! `input` example, extended to multiple lines, undo, and highlight ranges.

use std::ops::Range;

use gpui::{
    App, AvailableSpace, Bounds, ClipboardItem, Context, CursorStyle, ElementId,
    ElementInputHandler, Entity, EntityInputHandler, EventEmitter, FocusHandle, Focusable,
    GlobalElementId, Hsla, KeyBinding, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, PaintQuad, Pixels, Point, ScrollHandle, SharedString, Style, TextAlign, TextRun,
    UTF16Selection, UnderlineStyle, Window, WrappedLine, actions, div, fill, point, prelude::*, px,
    relative, size,
};

use crate::theme::Theme;

actions!(
    editor,
    [
        Backspace,
        Delete,
        DeleteWordLeft,
        Left,
        Right,
        Up,
        Down,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        WordLeft,
        WordRight,
        SelectWordLeft,
        SelectWordRight,
        Home,
        End,
        SelectHome,
        SelectEnd,
        DocumentStart,
        DocumentEnd,
        SelectAll,
        Newline,
        Copy,
        Cut,
        Paste,
        Undo,
        Redo,
        Cancel,
    ]
);

const KEY_CONTEXT: &str = "Editor";
const UNDO_LIMIT: usize = 500;

pub fn bind_keys(cx: &mut App) {
    let ctx = Some(KEY_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, ctx),
        KeyBinding::new("shift-backspace", Backspace, ctx),
        KeyBinding::new("delete", Delete, ctx),
        KeyBinding::new("left", Left, ctx),
        KeyBinding::new("right", Right, ctx),
        KeyBinding::new("up", Up, ctx),
        KeyBinding::new("down", Down, ctx),
        KeyBinding::new("shift-left", SelectLeft, ctx),
        KeyBinding::new("shift-right", SelectRight, ctx),
        KeyBinding::new("shift-up", SelectUp, ctx),
        KeyBinding::new("shift-down", SelectDown, ctx),
        KeyBinding::new("home", Home, ctx),
        KeyBinding::new("end", End, ctx),
        KeyBinding::new("shift-home", SelectHome, ctx),
        KeyBinding::new("shift-end", SelectEnd, ctx),
        KeyBinding::new("enter", Newline, ctx),
        KeyBinding::new("shift-enter", Newline, ctx),
        KeyBinding::new("escape", Cancel, ctx),
        KeyBinding::new("secondary-a", SelectAll, ctx),
        KeyBinding::new("secondary-c", Copy, ctx),
        KeyBinding::new("secondary-x", Cut, ctx),
        KeyBinding::new("secondary-v", Paste, ctx),
        KeyBinding::new("secondary-z", Undo, ctx),
        KeyBinding::new("secondary-shift-z", Redo, ctx),
    ]);
    #[cfg(target_os = "macos")]
    cx.bind_keys([
        KeyBinding::new("alt-left", WordLeft, ctx),
        KeyBinding::new("alt-right", WordRight, ctx),
        KeyBinding::new("alt-shift-left", SelectWordLeft, ctx),
        KeyBinding::new("alt-shift-right", SelectWordRight, ctx),
        KeyBinding::new("alt-backspace", DeleteWordLeft, ctx),
        KeyBinding::new("cmd-left", Home, ctx),
        KeyBinding::new("cmd-right", End, ctx),
        KeyBinding::new("cmd-shift-left", SelectHome, ctx),
        KeyBinding::new("cmd-shift-right", SelectEnd, ctx),
        KeyBinding::new("cmd-up", DocumentStart, ctx),
        KeyBinding::new("cmd-down", DocumentEnd, ctx),
    ]);
    #[cfg(not(target_os = "macos"))]
    cx.bind_keys([
        KeyBinding::new("ctrl-left", WordLeft, ctx),
        KeyBinding::new("ctrl-right", WordRight, ctx),
        KeyBinding::new("ctrl-shift-left", SelectWordLeft, ctx),
        KeyBinding::new("ctrl-shift-right", SelectWordRight, ctx),
        KeyBinding::new("ctrl-backspace", DeleteWordLeft, ctx),
        KeyBinding::new("ctrl-home", DocumentStart, ctx),
        KeyBinding::new("ctrl-end", DocumentEnd, ctx),
        KeyBinding::new("ctrl-y", Redo, ctx),
    ]);
}

/// A background color over a byte range of the editor's text.
#[derive(Clone, Debug, PartialEq)]
pub struct Highlight {
    pub range: Range<usize>,
    pub color: Hsla,
}

pub enum EditorEvent {
    Changed,
    /// Enter was pressed in a single-line editor.
    Submit,
    /// Escape was pressed.
    Cancel,
}

#[derive(Clone)]
struct Snapshot {
    content: String,
    selected_range: Range<usize>,
}

pub struct Editor {
    focus_handle: FocusHandle,
    content: String,
    placeholder: SharedString,
    read_only: bool,
    /// Enter submits instead of inserting a newline, and pasted newlines become spaces.
    single_line: bool,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    /// Sorted, non-overlapping background highlights.
    highlights: Vec<Highlight>,
    /// Offsets of zero-width matches, drawn as thin markers.
    markers: Vec<usize>,
    undo_stack: Vec<Snapshot>,
    redo_stack: Vec<Snapshot>,
    /// End offset of the last typed insertion, so consecutive typing undoes as one step.
    typing_at: Option<usize>,
    scroll_handle: Option<ScrollHandle>,
    autoscroll: bool,
    last_layout: Option<EditorLayout>,
    is_selecting: bool,
}

impl EventEmitter<EditorEvent> for Editor {}

impl Focusable for Editor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Editor {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            content: String::new(),
            placeholder: SharedString::default(),
            read_only: false,
            single_line: false,
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            highlights: Vec::new(),
            markers: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            typing_at: None,
            scroll_handle: None,
            autoscroll: false,
            last_layout: None,
            is_selecting: false,
        }
    }

    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn read_only(mut self) -> Self {
        self.read_only = true;
        self
    }

    pub fn single_line(mut self) -> Self {
        self.single_line = true;
        self
    }

    /// Selects all text, so typing replaces it.
    pub fn select_all_text(&mut self, cx: &mut Context<Self>) {
        self.selected_range = 0..self.content.len();
        self.selection_reversed = false;
        cx.notify();
    }

    /// Scroll container to keep the cursor visible in.
    pub fn scroll_handle(mut self, handle: ScrollHandle) -> Self {
        self.scroll_handle = Some(handle);
        self
    }

    pub fn text(&self) -> &str {
        &self.content
    }

    /// Replaces the whole text, clearing undo history. Does not emit [`EditorEvent::Changed`].
    pub fn set_text(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.content = text.into();
        self.selected_range = 0..0;
        self.selection_reversed = false;
        self.marked_range = None;
        self.highlights.clear();
        self.markers.clear();
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.typing_at = None;
        cx.notify();
    }

    pub fn set_highlights(
        &mut self,
        highlights: Vec<Highlight>,
        markers: Vec<usize>,
        cx: &mut Context<Self>,
    ) {
        if self.highlights != highlights || self.markers != markers {
            self.highlights = highlights;
            self.markers = markers;
            cx.notify();
        }
    }

    /// Selects a range and scrolls it into view.
    pub fn select_range(&mut self, range: Range<usize>, cx: &mut Context<Self>) {
        let end = self.content.len();
        self.selected_range = range.start.min(end)..range.end.min(end);
        self.selection_reversed = false;
        self.autoscroll = true;
        cx.notify();
    }

    pub fn move_to_end(&mut self, cx: &mut Context<Self>) {
        self.move_to(self.content.len(), cx);
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        self.typing_at = None;
        self.autoscroll = true;
        cx.notify();
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.selection_reversed {
            self.selected_range.start = offset;
        } else {
            self.selected_range.end = offset;
        }
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        self.typing_at = None;
        self.autoscroll = true;
        cx.notify();
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content[..offset]
            .char_indices()
            .next_back()
            .map_or(0, |(ix, _)| ix)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content[offset..]
            .chars()
            .next()
            .map_or(self.content.len(), |ch| offset + ch.len_utf8())
    }

    fn previous_word_start(&self, offset: usize) -> usize {
        let mut chars = self.content[..offset].char_indices().rev().peekable();
        while chars.next_if(|(_, ch)| ch.is_whitespace()).is_some() {}
        let Some(&(_, first)) = chars.peek() else {
            return 0;
        };
        let word = is_word_char(first);
        let mut start = offset;
        while let Some((ix, _)) =
            chars.next_if(|(_, ch)| !ch.is_whitespace() && is_word_char(*ch) == word)
        {
            start = ix;
        }
        start
    }

    fn next_word_end(&self, offset: usize) -> usize {
        let mut chars = self.content[offset..].char_indices().peekable();
        while chars.next_if(|(_, ch)| ch.is_whitespace()).is_some() {}
        let Some(&(_, first)) = chars.peek() else {
            return self.content.len();
        };
        let word = is_word_char(first);
        let mut end = offset;
        while let Some((ix, ch)) =
            chars.next_if(|(_, ch)| !ch.is_whitespace() && is_word_char(*ch) == word)
        {
            end = offset + ix + ch.len_utf8();
        }
        end
    }

    fn word_range_at(&self, offset: usize) -> Range<usize> {
        let at = |ix: usize| self.content[ix..].chars().next();
        let Some(ch) = at(offset).or_else(|| {
            let prev = self.previous_boundary(offset);
            at(prev)
        }) else {
            return offset..offset;
        };
        let class = |c: char| (c.is_whitespace(), is_word_char(c));
        let target = class(ch);
        let mut start = offset;
        while start > 0 {
            let prev = self.previous_boundary(start);
            if at(prev).map(class) != Some(target) {
                break;
            }
            start = prev;
        }
        let mut end = offset;
        while let Some(c) = at(end) {
            if class(c) != target {
                break;
            }
            end += c.len_utf8();
        }
        start..end
    }

    fn line_start(&self, offset: usize) -> usize {
        self.content[..offset].rfind('\n').map_or(0, |ix| ix + 1)
    }

    fn line_end(&self, offset: usize) -> usize {
        self.content[offset..]
            .find('\n')
            .map_or(self.content.len(), |ix| offset + ix)
    }

    /// Offset one visual row above (`rows < 0`) or below the given offset.
    fn vertical_offset(&self, offset: usize, rows: f32) -> usize {
        let Some(layout) = self.last_layout.as_ref() else {
            return offset;
        };
        let position = layout.position_for_offset(offset);
        let target = point(position.x, position.y + layout.line_height * (rows + 0.5));
        if target.y < Pixels::ZERO {
            0
        } else if target.y >= layout.total_height() {
            self.content.len()
        } else {
            self.offset_for_layout_point(layout, target)
        }
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx);
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.end, cx);
        }
    }

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.vertical_offset(self.cursor_offset(), -1.), cx);
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.vertical_offset(self.cursor_offset(), 1.), cx);
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.vertical_offset(self.cursor_offset(), -1.), cx);
    }

    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.vertical_offset(self.cursor_offset(), 1.), cx);
    }

    fn word_left(&mut self, _: &WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.previous_word_start(self.cursor_offset()), cx);
    }

    fn word_right(&mut self, _: &WordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.next_word_end(self.cursor_offset()), cx);
    }

    fn select_word_left(&mut self, _: &SelectWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_word_start(self.cursor_offset()), cx);
    }

    fn select_word_right(&mut self, _: &SelectWordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_word_end(self.cursor_offset()), cx);
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.line_start(self.cursor_offset()), cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.line_end(self.cursor_offset()), cx);
    }

    fn select_home(&mut self, _: &SelectHome, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.line_start(self.cursor_offset()), cx);
    }

    fn select_end(&mut self, _: &SelectEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.line_end(self.cursor_offset()), cx);
    }

    fn document_start(&mut self, _: &DocumentStart, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
    }

    fn document_end(&mut self, _: &DocumentEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.content.len(), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.selected_range = 0..self.content.len();
        self.selection_reversed = false;
        self.typing_at = None;
        cx.notify();
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.previous_boundary(self.cursor_offset()), cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.next_boundary(self.cursor_offset()), cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn delete_word_left(
        &mut self,
        _: &DeleteWordLeft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected_range.is_empty() {
            self.select_to(self.previous_word_start(self.cursor_offset()), cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn newline(&mut self, _: &Newline, window: &mut Window, cx: &mut Context<Self>) {
        if self.single_line {
            cx.emit(EditorEvent::Submit);
        } else {
            self.replace_text_in_range(None, "\n", window, cx);
        }
    }

    fn cancel(&mut self, _: &Cancel, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(EditorEvent::Cancel);
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() && !self.read_only {
            self.copy(&Copy, window, cx);
            self.replace_text_in_range(None, "", window, cx);
        }
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.typing_at = None;
            let mut text = text.replace("\r\n", "\n");
            if self.single_line {
                text = text.replace('\n', " ");
            }
            self.replace_text_in_range(None, &text, window, cx);
            self.typing_at = None;
        }
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            content: self.content.clone(),
            selected_range: self.selected_range.clone(),
        }
    }

    fn restore(&mut self, snapshot: Snapshot, cx: &mut Context<Self>) {
        self.content = snapshot.content;
        self.selected_range = snapshot.selected_range;
        self.selection_reversed = false;
        self.marked_range = None;
        self.typing_at = None;
        self.autoscroll = true;
        cx.emit(EditorEvent::Changed);
        cx.notify();
    }

    fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        if let Some(snapshot) = self.undo_stack.pop() {
            self.redo_stack.push(self.snapshot());
            self.restore(snapshot, cx);
        }
    }

    fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        if let Some(snapshot) = self.redo_stack.pop() {
            self.undo_stack.push(self.snapshot());
            self.restore(snapshot, cx);
        }
    }

    /// Replaces `range` with `new_text`, recording undo history and notifying subscribers.
    fn edit(&mut self, range: Range<usize>, new_text: &str, cx: &mut Context<Self>) {
        let is_typing = range.is_empty()
            && !new_text.is_empty()
            && !new_text.contains(char::is_whitespace)
            && self.typing_at == Some(range.start);
        if !is_typing {
            self.undo_stack.push(self.snapshot());
            if self.undo_stack.len() > UNDO_LIMIT {
                self.undo_stack.remove(0);
            }
        }
        self.redo_stack.clear();
        self.content.replace_range(range.clone(), new_text);
        let end = range.start + new_text.len();
        self.typing_at = (range.is_empty() && !new_text.is_empty()).then_some(end);
        self.autoscroll = true;
        cx.emit(EditorEvent::Changed);
        cx.notify();
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle);
        cx.stop_propagation();
        let offset = self.offset_for_window_point(event.position);
        match event.click_count {
            2 => {
                let range = self.word_range_at(offset);
                self.select_range(range, cx);
            }
            3 => {
                let start = self.line_start(offset);
                let end = self.next_boundary(self.line_end(offset));
                self.select_range(start..end, cx);
            }
            _ => {
                self.is_selecting = true;
                if event.modifiers.shift {
                    self.select_to(offset, cx);
                } else {
                    self.move_to(offset, cx);
                }
            }
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.offset_for_window_point(event.position), cx);
        }
    }

    fn offset_for_window_point(&self, position: Point<Pixels>) -> usize {
        let Some(layout) = self.last_layout.as_ref() else {
            return 0;
        };
        let local = position - layout.bounds.origin;
        if local.y < Pixels::ZERO {
            0
        } else if local.y >= layout.total_height() {
            self.content.len()
        } else {
            self.offset_for_layout_point(layout, local)
        }
    }

    /// Text offset at a point local to the last layout. An empty editor's layout
    /// shows the placeholder, so its offsets must not leak into the (empty) content.
    fn offset_for_layout_point(&self, layout: &EditorLayout, local: Point<Pixels>) -> usize {
        if self.content.is_empty() {
            0
        } else {
            layout.offset_for_point(local)
        }
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;
        for ch in self.content.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += ch.len_utf16();
            utf8_offset += ch.len_utf8();
        }
        utf8_offset
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        self.content[..offset].chars().map(char::len_utf16).sum()
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }
}

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

impl EntityInputHandler for Editor {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        self.edit(range.clone(), new_text, cx);
        let cursor = range.start + new_text.len();
        self.selected_range = cursor..cursor;
        self.selection_reversed = false;
        self.marked_range = None;
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        self.edit(range.clone(), new_text, cx);
        self.typing_at = None;
        self.marked_range =
            (!new_text.is_empty()).then(|| range.start..range.start + new_text.len());
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .map(|new_range| new_range.start + range.start..new_range.end + range.start)
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());
        self.selection_reversed = false;
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let layout = self.last_layout.as_ref()?;
        let range = self.range_from_utf16(&range_utf16);
        let start = layout.position_for_offset(range.start);
        let end = layout.position_for_offset(range.end);
        let end_x = if end.y == start.y { end.x } else { start.x };
        Some(Bounds::from_corners(
            element_bounds.origin + start,
            element_bounds.origin + point(end_x, start.y + layout.line_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let layout = self.last_layout.as_ref()?;
        let local = point - layout.bounds.origin;
        Some(self.offset_to_utf16(self.offset_for_layout_point(layout, local)))
    }
}

impl Render for Editor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut root = div()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .cursor(CursorStyle::IBeam)
            .w_full()
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::word_left))
            .on_action(cx.listener(Self::word_right))
            .on_action(cx.listener(Self::select_word_left))
            .on_action(cx.listener(Self::select_word_right))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::select_home))
            .on_action(cx.listener(Self::select_end))
            .on_action(cx.listener(Self::document_start))
            .on_action(cx.listener(Self::document_end))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::cancel))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move));
        if !self.read_only {
            root = root
                .on_action(cx.listener(Self::backspace))
                .on_action(cx.listener(Self::delete))
                .on_action(cx.listener(Self::delete_word_left))
                .on_action(cx.listener(Self::newline))
                .on_action(cx.listener(Self::cut))
                .on_action(cx.listener(Self::paste))
                .on_action(cx.listener(Self::undo))
                .on_action(cx.listener(Self::redo));
        }
        root.child(EditorElement {
            editor: cx.entity(),
        })
    }
}

struct LineLayout {
    /// Byte offset of this hard line within the editor text.
    start: usize,
    line: WrappedLine,
    y: Pixels,
}

/// Shaped text from the last paint, in coordinates local to the element.
struct EditorLayout {
    lines: Vec<LineLayout>,
    bounds: Bounds<Pixels>,
    line_height: Pixels,
}

impl EditorLayout {
    fn line_height_of(&self, line: &WrappedLine) -> Pixels {
        line.size(self.line_height).height
    }

    fn total_height(&self) -> Pixels {
        self.lines.last().map_or(self.line_height, |last| {
            last.y + self.line_height_of(&last.line)
        })
    }

    fn position_for_offset(&self, offset: usize) -> Point<Pixels> {
        for layout in &self.lines {
            if offset <= layout.start + layout.line.len() {
                let local = offset.saturating_sub(layout.start);
                let position = layout
                    .line
                    .position_for_index(local, self.line_height)
                    .unwrap_or_default();
                return point(position.x, layout.y + position.y);
            }
        }
        point(Pixels::ZERO, self.total_height() - self.line_height)
    }

    fn offset_for_point(&self, local: Point<Pixels>) -> usize {
        let Some(layout) = self
            .lines
            .iter()
            .find(|layout| local.y < layout.y + self.line_height_of(&layout.line))
            .or(self.lines.last())
        else {
            return 0;
        };
        let position = point(
            local.x.max(Pixels::ZERO),
            (local.y - layout.y).max(Pixels::ZERO),
        );
        let index = layout
            .line
            .closest_index_for_position(position, self.line_height)
            .unwrap_or_else(|index| index);
        layout.start + index
    }
}

struct EditorElement {
    editor: Entity<Editor>,
}

struct PrepaintState {
    layout: Option<EditorLayout>,
    /// Indices of the lines inside the scroll viewport.
    visible: Range<usize>,
    highlights: Vec<PaintQuad>,
    selection: Vec<PaintQuad>,
    markers: Vec<PaintQuad>,
    cursor: Option<PaintQuad>,
}

impl IntoElement for EditorElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl EditorElement {
    /// Text to display and its base run: the content, or the placeholder when empty.
    fn display_text(editor: &Editor, window: &Window, theme: &Theme) -> (SharedString, TextRun) {
        let style = window.text_style();
        let (text, color) = if editor.content.is_empty() {
            (editor.placeholder.clone(), theme.text_muted)
        } else {
            (SharedString::from(editor.content.clone()), style.color)
        };
        let run = TextRun {
            len: text.len(),
            font: style.font(),
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        (text, run)
    }
}

impl Element for EditorElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let theme = *cx.global::<Theme>();
        let (text, run) = Self::display_text(self.editor.read(cx), window, &theme);
        let font_size = window.text_style().font_size.to_pixels(window.rem_size());
        let line_height = window.line_height();

        let mut style = Style::default();
        style.size.width = relative(1.).into();
        let layout_id =
            window.request_measured_layout(style, move |known, available, window, _| {
                let wrap_width = known.width.or(match available.width {
                    AvailableSpace::Definite(width) => Some(width),
                    _ => None,
                });
                let lines = window
                    .text_system()
                    .shape_text(
                        text.clone(),
                        font_size,
                        std::slice::from_ref(&run),
                        wrap_width,
                        None,
                    )
                    .unwrap_or_default();
                let mut height = Pixels::ZERO;
                let mut width = Pixels::ZERO;
                for line in &lines {
                    let line_size = line.size(line_height);
                    height += line_size.height;
                    width = width.max(line_size.width);
                }
                size(wrap_width.unwrap_or(width), height.max(line_height))
            });
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let started = std::time::Instant::now();
        let theme = *cx.global::<Theme>();
        let editor = self.editor.read(cx);
        let (text, base_run) = Self::display_text(editor, window, &theme);
        // Highlights are painted as quads rather than run backgrounds, so shaping only
        // depends on the text and GPUI's line layout cache survives match changes.
        let runs = match editor.marked_range.as_ref() {
            Some(marked) if !editor.content.is_empty() => marked_runs(&base_run, marked),
            _ => vec![base_run],
        };
        let font_size = window.text_style().font_size.to_pixels(window.rem_size());
        let line_height = window.line_height();
        let wrapped = window
            .text_system()
            .shape_text(text, font_size, &runs, Some(bounds.size.width), None)
            .unwrap_or_default();

        let mut lines = Vec::with_capacity(wrapped.len());
        let mut start = 0;
        let mut y = Pixels::ZERO;
        for line in wrapped {
            let len = line.len();
            let height = line.size(line_height).height;
            lines.push(LineLayout { start, line, y });
            start += len + 1;
            y += height;
        }
        let layout = EditorLayout {
            lines,
            bounds,
            line_height,
        };

        // Large samples have thousands of lines; only those in the viewport are painted.
        let viewport = window.content_mask().bounds;
        let first = layout.lines.partition_point(|line| {
            bounds.top() + line.y + layout.line_height_of(&line.line) < viewport.top()
        });
        let last = layout
            .lines
            .partition_point(|line| bounds.top() + line.y <= viewport.bottom());
        let visible = first..last.max(first);

        let mut highlights = Vec::new();
        let mut selection = Vec::new();
        let mut markers = Vec::new();
        if !editor.content.is_empty() {
            for line in &layout.lines[visible.clone()] {
                let line_end = line.start + line.line.len();
                let first_highlight = editor
                    .highlights
                    .partition_point(|highlight| highlight.range.end <= line.start);
                for highlight in editor.highlights[first_highlight..]
                    .iter()
                    .take_while(|highlight| highlight.range.start <= line_end)
                {
                    line_range_quads(
                        &layout,
                        line,
                        &highlight.range,
                        highlight.color,
                        &mut highlights,
                    );
                }
                line_range_quads(
                    &layout,
                    line,
                    &editor.selected_range,
                    theme.selection,
                    &mut selection,
                );
                let first_marker = editor
                    .markers
                    .partition_point(|&offset| offset < line.start);
                for &offset in editor.markers[first_marker..]
                    .iter()
                    .take_while(|&&offset| offset <= line_end)
                {
                    let position = line
                        .line
                        .position_for_index(offset - line.start, line_height)
                        .unwrap_or_default();
                    markers.push(fill(
                        Bounds::new(
                            bounds.origin + point(position.x - px(1.), line.y + position.y),
                            size(px(2.), line_height),
                        ),
                        theme.empty_match,
                    ));
                }
            }
        }

        let cursor = editor.selected_range.is_empty().then(|| {
            let position = if editor.content.is_empty() {
                Point::default()
            } else {
                layout.position_for_offset(editor.cursor_offset())
            };
            fill(
                Bounds::new(bounds.origin + position, size(px(2.), line_height)),
                theme.cursor,
            )
        });

        log::trace!("editor prepaint took {:?}", started.elapsed());
        PrepaintState {
            layout: Some(layout),
            visible,
            highlights,
            selection,
            markers,
            cursor,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let editor = self.editor.read(cx);
        let focus_handle = editor.focus_handle.clone();
        let read_only = editor.read_only;
        let scroll_handle = editor.scroll_handle.clone();
        let autoscroll = editor.autoscroll;
        if !read_only {
            window.handle_input(
                &focus_handle,
                ElementInputHandler::new(bounds, self.editor.clone()),
                cx,
            );
        }

        let layout = prepaint
            .layout
            .take()
            .expect("prepaint always produces a layout");
        let line_height = layout.line_height;
        for quad in prepaint
            .highlights
            .drain(..)
            .chain(prepaint.selection.drain(..))
        {
            window.paint_quad(quad);
        }
        for line in &layout.lines[prepaint.visible.clone()] {
            let origin = bounds.origin + point(Pixels::ZERO, line.y);
            line.line
                .paint(origin, line_height, TextAlign::Left, None, window, cx)
                .ok();
        }
        for quad in prepaint.markers.drain(..) {
            window.paint_quad(quad);
        }

        let focused = focus_handle.is_focused(window);
        let cursor = prepaint.cursor.take();
        if focused && let Some(cursor) = cursor.clone() {
            window.paint_quad(cursor);
        }

        if autoscroll && let Some(handle) = scroll_handle {
            let viewport = handle.bounds();
            let target = cursor.map(|quad| quad.bounds).unwrap_or_else(|| {
                let editor = self.editor.read(cx);
                let position = layout.position_for_offset(editor.cursor_offset());
                Bounds::new(bounds.origin + position, size(px(2.), line_height))
            });
            let mut offset = handle.offset();
            if target.bottom() > viewport.bottom() {
                offset.y -= target.bottom() - viewport.bottom();
            } else if target.top() < viewport.top() {
                offset.y += viewport.top() - target.top();
            }
            if offset != handle.offset() {
                handle.set_offset(offset);
                window.refresh();
            }
        }

        self.editor.update(cx, |editor, _| {
            editor.last_layout = Some(layout);
            editor.autoscroll = false;
        });
    }
}

/// Splits the base run so the IME's in-progress (marked) text is underlined.
fn marked_runs(base: &TextRun, marked: &Range<usize>) -> Vec<TextRun> {
    let len = base.len;
    let (start, end) = (marked.start.min(len), marked.end.min(len));
    let underline = Some(UnderlineStyle {
        color: Some(base.color),
        thickness: px(1.),
        wavy: false,
    });
    [(start, None), (end - start, underline), (len - end, None)]
        .into_iter()
        .filter(|(len, _)| *len > 0)
        .map(|(len, underline)| TextRun {
            len,
            underline,
            ..base.clone()
        })
        .collect()
}

/// Appends quads covering the part of `range` that falls on `line`, which may wrap
/// onto several rows. A range continuing past the line end also covers its newline.
fn line_range_quads(
    layout: &EditorLayout,
    line: &LineLayout,
    range: &Range<usize>,
    color: Hsla,
    quads: &mut Vec<PaintQuad>,
) {
    let line_end = line.start + line.line.len();
    if range.is_empty() || range.end < line.start || range.start > line_end {
        return;
    }
    let origin = layout.bounds.origin;
    let width = layout.bounds.size.width;
    let line_height = layout.line_height;
    let position = |offset: usize| {
        line.line
            .position_for_index(offset - line.start, line_height)
            .unwrap_or_default()
    };
    let start = position(range.start.max(line.start));
    let end = position(range.end.min(line_end));
    let end_x = if range.end > line_end {
        end.x + px(6.)
    } else {
        end.x
    };
    let mut push = |x0: Pixels, x1: Pixels, y: Pixels| {
        if x1 > x0 {
            quads.push(fill(
                Bounds::from_corners(
                    origin + point(x0, line.y + y),
                    origin + point(x1, line.y + y + line_height),
                ),
                color,
            ));
        }
    };
    if start.y == end.y {
        push(start.x, end_x, start.y);
    } else {
        push(start.x, width, start.y);
        let mut y = start.y + line_height;
        while y < end.y {
            push(Pixels::ZERO, width, y);
            y += line_height;
        }
        push(Pixels::ZERO, end_x, end.y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{black, font};

    #[test]
    fn marked_runs_underline_only_the_marked_text() {
        let base = TextRun {
            len: 10,
            font: font("Menlo"),
            color: black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = marked_runs(&base, &(3..8));
        let lens: Vec<usize> = runs.iter().map(|run| run.len).collect();
        assert_eq!(lens, vec![3, 5, 2]);
        let underlined: Vec<bool> = runs.iter().map(|run| run.underline.is_some()).collect();
        assert_eq!(underlined, vec![false, true, false]);

        let runs = marked_runs(&base, &(0..10));
        assert_eq!(runs.len(), 1);
        assert!(runs[0].underline.is_some());
    }
}
