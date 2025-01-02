use cli_log::*;
use std::collections::VecDeque;

use ratatui::text::Line;
use ratatui::prelude::Rect;
use ratatui::prelude::Buffer;

pub enum RenderingTop {
    Top,
    Bottom,
}

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

    fn render(&mut self, rect: Rect, buff: &mut Buffer, selected: bool, top: RenderingTop) {
        self.set_cache(rect.width, selected);

        match top {
            RenderingTop::Top => {
                for (idx, line) in self.get_cache().as_ref().unwrap().cache.iter().rev().enumerate() {
                    if idx as u16 >= rect.height {
                        break;
                    }

                    buff.set_line(rect.x, rect.y + rect.height - 1 - idx as u16, line, rect.width);
                }
            },
            RenderingTop::Bottom => {
                for (idx, line) in self.get_cache().as_ref().unwrap().cache.iter().enumerate() {
                    if idx as u16 >= rect.height {
                        break;
                    }

                    buff.set_line(rect.x, rect.y + idx as u16, line, rect.width);
                }
            }
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
    top_visisted: Option<(u16, u16)>,     // Simpler offset, makes implementing scrolling easier.
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
                top_visisted: None,
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
                self.scroll.top_visisted = Some((self.get_top_idx(), 0));

                return true;
            }

            return false;
        };

        if self.list.len() > (*selected_msg + 1).into() {
            if *selected_msg == self.scroll.top_visisted.unwrap().0 {
                self.scroll.top_visisted = Some((*selected_msg + 1, 0));
            }
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
        self.scroll.top_visisted = None;
    }

    pub fn select(&mut self, idx: u16) {
        self.scroll.selected_msg = Some(idx.max(self.list.len() as u16 - 1));
        self.scroll.top_visisted = Some((idx.max(self.list.len() as u16 - 1), 0));
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

        // Calculating offset of list item rendering. Could be improved.
        match (self.scroll.selected_msg, self.scroll.top_visisted) {
            (Some(selected_msg), Some(top_visisted)) => {
                trace!("sel:{} top:{:?}", selected_msg, top_visisted);
                if selected_msg > top_visisted.0 {
                    for i in (top_visisted.0..=selected_msg).rev() {
                        if height_sum == rect.height {
                            break;
                        }

                        self.list[i as usize].set_cache(rect.width, i == selected_msg);
                        let lines_cnt = self.list[i as usize].get_cache().as_ref().unwrap().height;
                        let used_lines = match i {
                            x if x == top_visisted.0 && x != selected_msg => {
                                lines_cnt
                                    .min(lines_cnt.max(top_visisted.1) - top_visisted.1)
                                    .min(rect.height - height_sum)
                            }
                            _ => {
                                lines_cnt.min(rect.height - height_sum)
                            }
                        };

                        height_sum += used_lines;
                        items.push_front((i, used_lines));
                        self.scroll.top_visisted = Some((i, lines_cnt - used_lines));
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

                    if top_visisted.0 > 0 {
                        for i in (0..=top_visisted.0 - 1).rev() {
                            if height_sum == rect.height {
                                break;
                            }

                            self.list[i as usize].set_cache(rect.width, i == selected_msg);
                            let lines_cnt = self.list[i as usize].get_cache().as_ref().unwrap().height;
                            let used_lines = lines_cnt.min(rect.height - height_sum);
                            height_sum += used_lines;
                            items.push_front((i, used_lines));

                            self.scroll.top_visisted = Some((i, lines_cnt - used_lines));
                        }
                    }
                } else {
                    for i in selected_msg..=top_visisted.0 {
                        if height_sum == rect.height {
                            break;
                        }

                        self.list[i as usize].set_cache(rect.width, i == selected_msg);
                        let lines_cnt = self.list[i as usize].get_cache().as_ref().unwrap().height;
                        let used_lines = match i {
                            x if x == top_visisted.0 && x != selected_msg => {
                                lines_cnt
                                    .min(lines_cnt.max(top_visisted.1) - top_visisted.1)
                                    .min(rect.height - height_sum)
                            }
                            _ => {
                                lines_cnt.min(rect.height - height_sum)
                            }
                        };

                        height_sum += used_lines;
                        items.push_back((i, used_lines));
                        self.scroll.top_visisted = Some((i, lines_cnt - used_lines));
                    }

                    if selected_msg > 0 {
                        for i in (0..=selected_msg - 1).rev() {
                            if height_sum == rect.height {
                                break;
                            }

                            self.list[i as usize].set_cache(rect.width, i == selected_msg);
                            let used_lines = self.list[i as usize].get_cache().as_ref().unwrap().height.min(rect.height - height_sum);
                            height_sum += used_lines;
                            items.push_front((i, used_lines));
                        }
                    }

                    for i in top_visisted.0 + 1..self.list.len() as u16 {
                        if height_sum == rect.height {
                            break;
                        }

                        self.list[i as usize].set_cache(rect.width, i == selected_msg);
                        let lines_cnt = self.list[i as usize].get_cache().as_ref().unwrap().height;
                        let used_lines = lines_cnt.min(rect.height - height_sum);
                        height_sum += used_lines;
                        items.push_back((i, used_lines));
                        
                        self.scroll.top_visisted = Some((i, lines_cnt - used_lines));
                    }
                }
            }
            _ => {
                match self.scroll.top {
                    ListTop::First => {
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
                    ListTop::Last => {
                        for i in (0..self.list.len() as u16).rev() {
                            if height_sum == rect.height {
                                break;
                            }

                            self.list[i as usize].set_cache(rect.width, false);
                            let used_lines = self.list[i as usize].get_cache().as_ref().unwrap().height.min(rect.height - height_sum);
                            height_sum += used_lines;
                            items.push_front((i, used_lines));
                        }
                    }
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
                let mut direction = RenderingTop::Top;

                for item in items_iterator {
                    self.list[item.0 as usize].render(
                        Rect::new(rect.x, rect.y + height_sum, rect.width, item.1),
                        buff,
                        self.scroll.selected_msg.is_some_and(|idx| idx == item.0),
                        direction,
                    );
                    height_sum += item.1;
                    direction = RenderingTop::Bottom;
                }

                for height in height_sum..rect.height {
                    buff.set_line(rect.x, rect.y + height, &Line::from(" ".repeat(rect.width.into())), rect.width);
                }
            },
            ListBegin::Bottom => {
                let mut direction = RenderingTop::Bottom;

                for item in items_iterator {
                    self.list[item.0 as usize].render(
                        Rect::new(rect.x, rect.y + rect.height - height_sum - item.1, rect.width, item.1),
                        buff,
                        self.scroll.selected_msg.is_some_and(|idx| idx == item.0),
                        direction,
                    );
                    height_sum += item.1;
                    direction = RenderingTop::Top;
                }

                for height in height_sum..rect.height {
                    buff.set_line(rect.x, rect.y + height - height_sum, &Line::from(" ".repeat(rect.width.into())), rect.width);
                }
            }
        }
    }
}