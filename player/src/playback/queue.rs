use crate::domain::{RepeatMode, Song};

#[derive(Debug, Default, Clone)]
pub struct Queue {
    pub items: Vec<Song>,
    pub current: Option<usize>,
    pub shuffle: bool,
    pub repeat: RepeatMode,
}

impl Queue {
    pub fn replace(&mut self, songs: Vec<Song>, start: usize) {
        if songs.is_empty() {
            self.items.clear();
            self.current = None;
            return;
        }
        let start = start.min(songs.len() - 1);
        self.items = songs;
        self.current = Some(start);
    }

    pub fn current_song(&self) -> Option<&Song> {
        self.current.and_then(|i| self.items.get(i))
    }

    pub fn next_index(&self) -> Option<usize> {
        let cur = self.current?;
        if self.items.is_empty() {
            return None;
        }
        match self.repeat {
            RepeatMode::One => Some(cur),
            RepeatMode::All => Some((cur + 1) % self.items.len()),
            RepeatMode::Off => {
                if cur + 1 < self.items.len() {
                    Some(cur + 1)
                } else {
                    None
                }
            }
        }
    }

    pub fn prev_index(&self) -> Option<usize> {
        let cur = self.current?;
        if self.items.is_empty() {
            return None;
        }
        if cur == 0 {
            match self.repeat {
                RepeatMode::All => Some(self.items.len() - 1),
                _ => Some(0),
            }
        } else {
            Some(cur - 1)
        }
    }

    pub fn advance(&mut self) -> Option<&Song> {
        let next = self.next_index()?;
        self.current = Some(next);
        self.items.get(next)
    }

    pub fn rewind(&mut self) -> Option<&Song> {
        let prev = self.prev_index()?;
        self.current = Some(prev);
        self.items.get(prev)
    }

    pub fn jump_to(&mut self, index: usize) -> Option<&Song> {
        if index >= self.items.len() {
            return None;
        }
        self.current = Some(index);
        self.items.get(index)
    }

    pub fn remove_song_id(&mut self, song_id: i64) {
        let mut removed_before_current = 0usize;
        let cur = self.current;
        let mut new_items = Vec::with_capacity(self.items.len());
        for (i, s) in self.items.drain(..).enumerate() {
            if s.id == song_id {
                if let Some(c) = cur {
                    if i < c {
                        removed_before_current += 1;
                    } else if i == c {
                        // current removed; we'll fixup below
                    }
                }
                continue;
            }
            new_items.push(s);
        }
        self.items = new_items;
        if self.items.is_empty() {
            self.current = None;
            return;
        }
        self.current = match cur {
            Some(c) => {
                let new_c = c.saturating_sub(removed_before_current);
                Some(new_c.min(self.items.len() - 1))
            }
            None => None,
        };
    }
}
