//! Project rail shared by the folder launcher and the management window.
//!
//! Up to six projects are visible. The active project is always visible; the
//! remaining projects move into a keyboard-accessible overflow menu.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use uuid::Uuid;

use crate::wheel::ProjectTab;
use crate::widgets::color_dot;

const MAX_VISIBLE: usize = 5;

type SelectHandler = Rc<dyn Fn(Uuid)>;

#[derive(Clone)]
pub struct ProjectTabs {
    pub container: gtk::Box,
    tabs: gtk::Box,
    more: gtk::MenuButton,
    more_label: gtk::Label,
    overflow: gtk::Box,
    handler: Rc<RefCell<Option<SelectHandler>>>,
}

impl ProjectTabs {
    pub fn new() -> Self {
        let container = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        container.add_css_class("rd-tabs");
        container.set_halign(gtk::Align::Center);

        let tabs = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        container.append(&tabs);

        let more = gtk::MenuButton::new();
        more.add_css_class("rd-tab");
        more.add_css_class("rd-tab-more");
        more.set_always_show_arrow(false);
        more.set_tooltip_text(Some("More projects"));
        let more_label = gtk::Label::new(None);
        more_label.add_css_class("rd-tab-label");
        more.set_child(Some(&more_label));

        let overflow = gtk::Box::new(gtk::Orientation::Vertical, 2);
        overflow.add_css_class("rd-menu");
        overflow.set_margin_top(6);
        overflow.set_margin_bottom(6);
        overflow.set_margin_start(6);
        overflow.set_margin_end(6);
        let popover = gtk::Popover::new();
        popover.add_css_class("rd-menu-popover");
        popover.set_child(Some(&overflow));
        more.set_popover(Some(&popover));
        container.append(&more);

        Self {
            container,
            tabs,
            more,
            more_label,
            overflow,
            handler: Rc::new(RefCell::new(None)),
        }
    }

    pub fn set_handler(&self, handler: SelectHandler) {
        *self.handler.borrow_mut() = Some(handler);
    }

    /// Moves keyboard focus to the first visible project, or to the overflow
    /// button when no project tab is rendered.
    pub fn focus_first(&self) {
        if let Some(child) = self.tabs.first_child() {
            child.grab_focus();
        } else if self.more.get_visible() {
            self.more.grab_focus();
        }
    }

    pub fn render(&self, projects: &[ProjectTab], active: Uuid) {
        while let Some(child) = self.tabs.first_child() {
            self.tabs.remove(&child);
        }
        while let Some(child) = self.overflow.first_child() {
            self.overflow.remove(&child);
        }

        let visible = visible_projects(projects, active);
        for project in projects
            .iter()
            .filter(|project| visible.contains(&project.id))
        {
            let button = self.project_button(project, project.id == active, false);
            self.tabs.append(&button);
        }

        let hidden: Vec<_> = projects
            .iter()
            .filter(|project| !visible.contains(&project.id))
            .collect();
        self.more.set_visible(!hidden.is_empty());
        self.more_label.set_text(&format!("+{}", hidden.len()));
        for project in hidden {
            let button = self.project_button(project, project.id == active, true);
            self.overflow.append(&button);
        }
    }

    fn project_button(&self, project: &ProjectTab, active: bool, in_menu: bool) -> gtk::Button {
        let button = gtk::Button::new();
        button.add_css_class(if in_menu { "rd-menu-item" } else { "rd-tab" });
        if active {
            button.add_css_class("active");
        }
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 7);
        content.append(&color_dot(&project.color, if in_menu { 10 } else { 9 }));
        let label = gtk::Label::new(Some(&project.name));
        label.add_css_class("rd-tab-label");
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        label.set_max_width_chars(20);
        content.append(&label);
        if in_menu {
            let count = gtk::Label::new(Some(&format!(
                "{} folder{}",
                project.folder_count,
                if project.folder_count == 1 { "" } else { "s" }
            )));
            count.add_css_class("rd-menu-meta");
            count.set_hexpand(true);
            count.set_halign(gtk::Align::End);
            content.append(&count);
        }
        button.set_child(Some(&content));
        button.set_focusable(true);
        button.set_tooltip_text(Some(&format!(
            "{} · {} folder{}",
            project.name,
            project.folder_count,
            if project.folder_count == 1 { "" } else { "s" }
        )));

        let handler = self.handler.clone();
        let id = project.id;
        let more = self.more.clone();
        button.connect_clicked(move |_| {
            more.popdown();
            let handler = handler.clone();
            // Defer refresh until the button's signal emission has finished.
            gtk::glib::idle_add_local_once(move || {
                if let Some(handler) = handler.borrow().as_ref() {
                    handler(id);
                }
            });
        });
        button
    }
}

fn visible_projects(projects: &[ProjectTab], active: Uuid) -> Vec<Uuid> {
    if projects.len() <= MAX_VISIBLE {
        return projects.iter().map(|project| project.id).collect();
    }
    let active_index = projects
        .iter()
        .position(|project| project.id == active)
        .unwrap_or(0);
    let mut indices: Vec<usize> = (0..MAX_VISIBLE).collect();
    if !indices.contains(&active_index) {
        indices[MAX_VISIBLE - 1] = active_index;
    }
    indices
        .into_iter()
        .filter_map(|index| projects.get(index).map(|project| project.id))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wheel::ProjectTab;

    fn project(name: &str) -> ProjectTab {
        ProjectTab {
            id: Uuid::new_v4(),
            name: name.to_string(),
            color: String::new(),
            folder_count: 0,
        }
    }

    #[test]
    fn visible_projects_fills_the_rail_when_it_fits() {
        let projects: Vec<_> = (0..MAX_VISIBLE)
            .map(|i| project(&format!("P{i}")))
            .collect();
        let visible = visible_projects(&projects, projects[0].id);
        assert_eq!(visible, projects.iter().map(|p| p.id).collect::<Vec<_>>());
    }

    #[test]
    fn visible_projects_keeps_a_hidden_active_project_in_the_last_slot() {
        let projects: Vec<_> = (0..MAX_VISIBLE + 3)
            .map(|i| project(&format!("P{i}")))
            .collect();
        let active = projects[MAX_VISIBLE + 1].id;
        let visible = visible_projects(&projects, active);

        assert_eq!(visible.len(), MAX_VISIBLE);
        assert_eq!(visible[MAX_VISIBLE - 1], active);
        assert!(!visible.contains(&projects[MAX_VISIBLE - 1].id));
    }
}
