//! Shared controls. Every control has a fixed height, and controls whose label
//! changes (filters, segments) also have a fixed width, so nothing reflows when
//! a selection changes.
use super::*;
use gpui_component::{
    Sizable,
    button::{Button, ButtonCustomVariant, ButtonVariants},
};

pub struct Segment {
    pub label: SharedString,
    pub active: bool,
    pub action: UiAction,
    pub tooltip: Option<SharedString>,
}
impl Segment {
    pub fn new(label: impl Into<SharedString>, active: bool, action: UiAction) -> Self {
        Self {
            label: label.into(),
            active,
            action,
            tooltip: None,
        }
    }
    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }
}

impl Dashboard {
    fn variant(&self, active: bool, cx: &Context<Self>) -> ButtonCustomVariant {
        let theme = self.theme();
        ButtonCustomVariant::new(cx)
            .color(rgb(if active { theme.wash } else { theme.panel }).into())
            .foreground(rgb(theme.ink).into())
            .border(rgb(theme.line).into())
            .hover(rgb(theme.wash).into())
            .active(rgb(theme.wash).into())
    }
    /// A bordered button. Labels and icons carry their own color so hover never
    /// recolors them.
    pub(super) fn button(
        &self,
        id: impl Into<String>,
        text: impl Into<String>,
        glyph: Option<&'static str>,
        active: bool,
        action: UiAction,
        cx: &Context<Self>,
    ) -> Button {
        let theme = self.theme();
        let text = text.into();
        Button::new(SharedString::from(id.into()))
            .small()
            .h(px(CONTROL_HEIGHT))
            .rounded(px(RADIUS))
            .when(text.is_empty(), |button| button.w(px(CONTROL_HEIGHT)).px(px(0.)))
            .custom(self.variant(active, cx))
            .when_some(glyph, |button, glyph| {
                button.child(icon(glyph, theme).size(px(15.)).text_color(rgb(theme.ink)))
            })
            .when(!text.is_empty(), |button| {
                button.child(
                    div()
                        .text_size(px(12.))
                        .line_height(relative(1.))
                        .text_color(rgb(theme.ink))
                        .when(active, |text| text.font_weight(FontWeight::MEDIUM))
                        .child(text),
                )
            })
            .on_click(
                cx.listener(move |view, _, window, cx| view.perform(action.clone(), window, cx)),
            )
    }
    pub(super) fn icon_button(
        &self,
        id: impl Into<String>,
        glyph: &'static str,
        tooltip: &'static str,
        action: UiAction,
        cx: &Context<Self>,
    ) -> Button {
        self.button(id, "", Some(glyph), false, action, cx)
            .tooltip(tooltip)
    }
    /// A fixed-width dropdown trigger. Pair with `DropdownMenu::dropdown_menu`.
    pub(super) fn select(
        &self,
        id: impl Into<String>,
        text: impl Into<SharedString>,
        width: f32,
        active: bool,
        cx: &Context<Self>,
    ) -> Button {
        let theme = self.theme();
        Button::new(SharedString::from(id.into()))
            .small()
            .h(px(CONTROL_HEIGHT))
            .w(px(width))
            .rounded(px(RADIUS))
            .custom(self.variant(active, cx))
            .child(
                div()
                    .w(px(width - 42.))
                    .truncate()
                    .text_size(px(12.))
                    .line_height(relative(1.))
                    .text_color(rgb(if active { theme.ink } else { theme.muted }))
                    .when(active, |text| text.font_weight(FontWeight::MEDIUM))
                    .child(text.into()),
            )
            .child(icon("chevron", theme).size(px(12.)).text_color(rgb(theme.muted)))
    }
    /// Equal-width text tabs. The active one is marked by ink and a short rule
    /// underneath rather than a filled chip, so toolbars stay quiet.
    pub(super) fn segmented(
        &self,
        id: &str,
        segments: Vec<Segment>,
        segment_width: f32,
        cx: &Context<Self>,
    ) -> Div {
        let theme = self.theme();
        div()
            .h(px(CONTROL_HEIGHT))
            .flex_shrink_0()
            .flex()
            .items_center()
            .children(segments.into_iter().enumerate().map(|(index, segment)| {
                let action = segment.action.clone();
                let active = segment.active;
                div()
                    .id(SharedString::from(format!("{id}-{index}")))
                    .w(px(segment_width))
                    .h_full()
                    .flex_shrink_0()
                    .relative()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .text_size(px(12.))
                    .when(active, |tab| {
                        tab.text_color(rgb(theme.ink))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(
                                div()
                                    .absolute()
                                    .bottom(px(2.))
                                    .left(px(10.))
                                    .right(px(10.))
                                    .h(px(2.))
                                    .bg(rgb(theme.ink)),
                            )
                    })
                    .when(!active, |tab| {
                        tab.text_color(rgb(theme.faint))
                            .hover(move |style| style.text_color(rgb(theme.ink)))
                    })
                    .when_some(segment.tooltip, |chip, tooltip| {
                        chip.tooltip(move |window, cx| {
                            gpui_component::tooltip::Tooltip::new(tooltip.clone())
                                .build(window, cx)
                        })
                    })
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.perform(action.clone(), window, cx)
                    }))
                    .child(segment.label)
            }))
    }
    pub(super) fn period_control(&self, narrow: bool, cx: &Context<Self>) -> Div {
        self.segmented(
            "period",
            Period::ALL
                .into_iter()
                .map(|period| {
                    Segment::new(
                        period.key(),
                        self.period == period,
                        UiAction::Period(period),
                    )
                    .tooltip(period.label())
                })
                .collect(),
            if narrow { 34. } else { 40. },
            cx,
        )
    }
    /// Small muted text action, used for secondary links such as "Reset sort".
    pub(super) fn text_action(
        &self,
        id: impl Into<String>,
        text: &'static str,
        glyph: Option<&'static str>,
        action: UiAction,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let theme = self.theme();
        div()
            .id(SharedString::from(id.into()))
            .h(px(22.))
            .px(px(6.))
            .rounded(px(RADIUS - 2.))
            .flex()
            .items_center()
            .gap(px(4.))
            .cursor_pointer()
            .text_size(px(12.))
            .text_color(rgb(theme.muted))
            .hover(move |style| style.bg(rgb(theme.wash)).text_color(rgb(theme.ink)))
            .when_some(glyph, |row, glyph| {
                row.child(icon(glyph, theme).size(px(12.)).text_color(rgb(theme.muted)))
            })
            .child(text)
            .on_click(
                cx.listener(move |view, _, window, cx| view.perform(action.clone(), window, cx)),
            )
    }
    pub(super) fn sort_of(&self, table: Table) -> Sort {
        match table {
            Table::Models => self.model_sort,
            Table::Activity => self.activity_sort,
            Table::Projects => self.project_sort,
        }
    }
    /// A clickable column heading. The arrow slot is always reserved, so the
    /// heading never shifts when the sort changes.
    pub(super) fn sort_header(
        &self,
        table: Table,
        column: Column,
        width: Option<f32>,
        numeric: bool,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let theme = self.theme();
        let arrow = self.sort_of(table).arrow(column);
        let color = if arrow.is_some() {
            theme.ink
        } else {
            theme.muted
        };
        let glyph = div().w(px(12.)).flex_shrink_0().when_some(arrow, |slot, descending| {
            slot.child(
                icon(if descending { "sort-down" } else { "sort-up" }, theme)
                    .size(px(12.))
                    .text_color(rgb(theme.ink)),
            )
        });
        let cell = div()
            .id(SharedString::from(format!("sort-{table:?}-{column:?}")))
            .h_full()
            .flex()
            .items_center()
            .gap(px(3.))
            .cursor_pointer()
            .text_size(px(12.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(color))
            .hover(move |style| style.text_color(rgb(theme.ink)))
            .when_some(width, |cell, width| cell.w(px(width)).flex_shrink_0())
            .on_click(cx.listener(move |view, _, window, cx| {
                view.perform(UiAction::Sort(table, column), window, cx)
            }));
        let text = div().truncate().child(column.label());
        if numeric {
            cell.justify_end().child(glyph).child(text)
        } else {
            cell.child(text).child(glyph)
        }
    }
}
