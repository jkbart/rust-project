use std::collections::VecDeque;

use ratatui::text::Line;
use ratatui::prelude::Rect;
use ratatui::prelude::Buffer;

pub struct ListCache {
    cache: Vec<Line<'static>>,
    width: u16,
    height: u16,
    selected: bool,
}

impl ListCache {
    pub fn new(
        cache: Vec<Line<'static>>,
        width: u16,
        height: u16,
        selected: bool,
    ) -> Self {
        Self {
            cache,
            width,
            height,
            selected,
        }
    }
}

pub trait ListItem {
    fn get_cache(&mut self) -> &mut Option<ListCache>;
    fn prerender(&mut self, window_max_width: u16, selected: bool);

    fn set_cache(&mut self, window_max_width: u16, selected: bool) {
        let mut valid_cache = true;

        if let Some(cache) = &self.get_cache() {
            if (cache.width, cache.selected) != (window_max_width, selected) {
                valid_cache = false;
            }
        } else {
            valid_cache = false;
        }

        if !valid_cache {
            self.prerender(window_max_width, selected);
        }
    }

    fn render(&mut self, rect: Rect, buff: &mut Buffer, selected: bool) {
        self.set_cache(rect.width, selected);

        for (idx, line) in self.get_cache().as_ref().unwrap().cache.iter().enumerate() {
            if idx as u16 >= rect.height {
                break;
            }

            buff.set_line(rect.x, rect.y + idx as u16, line, rect.width);
        }
    }
}

pub enum ListBegin {
    Top,
    Bottom,
}

pub enum ListTop {
    First,
    Last,
}

struct Scroll {
    begining: ListBegin,
    top: ListTop,
    selected_msg: Option<u16>,
    last_selected: Option<u16>,     // Simpler offset, makes scrolling easier.
}

pub struct ListComponent<Item: ListItem> {
    pub list: Vec<Item>,        // Assuming this vector is only appended.
    scroll: Scroll,
}

impl<Item: ListItem> ListComponent<Item> {
    pub fn new(scroll_begin: ListBegin, list_top: ListTop) -> Self {
        Self {
            list: Vec::new(),
            scroll: Scroll {
                begining: scroll_begin,
                top: list_top,
                selected_msg: None,
                last_selected: None,
            },
        }
    }

    fn get_top_idx(&mut self) -> u16 {
        match self.scroll.top {
            ListTop::First => 0,
            ListTop::Last => self.list.len() as u16 - 1
        }
    }

    // Go down the list.
    pub fn go_down(&mut self) -> bool {
        let Some(selected_msg) = &mut self.scroll.selected_msg else {
            if !self.list.is_empty() {
                self.scroll.selected_msg = Some(self.get_top_idx());
                self.scroll.last_selected = Some(self.get_top_idx());

                return true;
            }

            return false;
        };

        if self.list.len() > (*selected_msg + 1).into() {
            *selected_msg += 1;
            return true;
        }

        return false;
    }

    // Go up the list.
    pub fn go_up(&mut self) -> bool {
        let Some(selected_msg) = &mut self.scroll.selected_msg else {
            return false;
        };
        
        if *selected_msg > 0 {
            *selected_msg -= 1;
            return true;
        }

        return false;
    }

    pub fn reset(&mut self) {
        self.scroll.selected_msg = None;
        self.scroll.last_selected = None;
    }

    pub fn select(&mut self, idx: u16) {
        self.scroll.selected_msg = Some(idx.max(self.list.len() as u16 - 1));
    }

    pub fn get_select_idx(&mut self) -> Option<u16> {
        self.scroll.selected_msg
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    pub fn push(&mut self, item: Item) {
        self.list.push(item);
    }

    pub fn append(&mut self, other: &mut Vec<Item>) {
        self.list.append(other);
    }

    pub fn render(&mut self, rect: Rect, buff: &mut Buffer) {
        let mut height_sum: u16 = 0;

        let mut items: VecDeque<(u16, u16)> = VecDeque::new();  // Index of item, number of lines rendered

        match (self.scroll.selected_msg, self.scroll.last_selected) {
            (Some(selected_msg), Some(last_selected)) => {
                if selected_msg > last_selected {
                    for i in (last_selected..=selected_msg).rev() {
                        if height_sum == rect.height {
                            break;
                        }

                        self.list[i as usize].set_cache(rect.width, i == selected_msg);
                        let used_lines = self.list[i as usize].get_cache().as_ref().unwrap().height.min(rect.height - height_sum);
                        height_sum += used_lines;
                        items.push_front((i, used_lines));
                    }

                    for i in selected_msg + 1..self.list.len() as u16 {
                        if height_sum == rect.height {
                            break;
                        }

                        self.list[i as usize].set_cache(rect.width, i == selected_msg);
                        let used_lines = self.list[i as usize].get_cache().as_ref().unwrap().height.min(rect.height - height_sum);
                        height_sum += used_lines;
                        items.push_back((i, used_lines));
                    }

                    for i in (0..=last_selected - 1).rev() {
                        if height_sum == rect.height {
                            break;
                        }

                        self.list[i as usize].set_cache(rect.width, i == selected_msg);
                        let used_lines = self.list[i as usize].get_cache().as_ref().unwrap().height.min(rect.height - height_sum);
                        height_sum += used_lines;
                        items.push_front((i, used_lines));
                    }
                } else {
                    for i in selected_msg..=last_selected {
                        if height_sum == rect.height {
                            break;
                        }

                        self.list[i as usize].set_cache(rect.width, i == selected_msg);
                        let used_lines = self.list[i as usize].get_cache().as_ref().unwrap().height.min(rect.height - height_sum);
                        height_sum += used_lines;
                        items.push_front((i, used_lines));
                    }

                    for i in (0..=last_selected - 1).rev() {
                        if height_sum == rect.height {
                            break;
                        }

                        self.list[i as usize].set_cache(rect.width, i == selected_msg);
                        let used_lines = self.list[i as usize].get_cache().as_ref().unwrap().height.min(rect.height - height_sum);
                        height_sum += used_lines;
                        items.push_front((i, used_lines));
                    }

                    for i in selected_msg + 1..self.list.len() as u16 {
                        if height_sum == rect.height {
                            break;
                        }

                        self.list[i as usize].set_cache(rect.width, i == selected_msg);
                        let used_lines = self.list[i as usize].get_cache().as_ref().unwrap().height.min(rect.height - height_sum);
                        height_sum += used_lines;
                        items.push_back((i, used_lines));
                    }
                }
            }
            _ => {
                for i in 0..self.list.len() as u16 {
                    if height_sum == rect.height {
                        break;
                    }

                    self.list[i as usize].set_cache(rect.width, false);
                    let used_lines = self.list[i as usize].get_cache().as_ref().unwrap().height.min(rect.height - height_sum);
                    height_sum += used_lines;
                    items.push_back((i, used_lines));
                }
            }
        }

        let items_iterator: Box<dyn Iterator<Item = &(u16, u16)>> = match self.scroll.top {
            ListTop::First => Box::new(items.iter()),
            ListTop::Last => Box::new(items.iter().rev()),
        };

        height_sum = 0;

        match self.scroll.begining {
            ListBegin::Top => {
                for item in items_iterator {
                    self.list[item.0 as usize].render(
                        Rect::new(rect.x, rect.y + height_sum, rect.width, item.1),
                        buff,
                        self.scroll.selected_msg.is_some_and(|idx| idx == item.0)
                    );
                    height_sum += item.1;
                }

                for height in height_sum..rect.height {
                    buff.set_line(rect.x, rect.y + height, &Line::from(" ".repeat(rect.width.into())), rect.width);
                }
            },
            ListBegin::Bottom => {
                for item in items_iterator {
                    self.list[item.0 as usize].render(
                        Rect::new(rect.x, rect.y + rect.height - height_sum - item.1, rect.width, item.1),
                        buff,
                        self.scroll.selected_msg.is_some_and(|idx| idx == item.0)
                    );
                    height_sum += item.1;
                }

                for height in height_sum..rect.height {
                    buff.set_line(rect.x, rect.y + height - height_sum, &Line::from(" ".repeat(rect.width.into())), rect.width);
                }
            }
        }
    }
}