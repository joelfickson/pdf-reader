use crate::pdf::tools::{Outline, OutlineEntry};
use std::{collections::BTreeSet, sync::Arc};

pub(super) struct OutlineTree {
    pub outline: Arc<Outline>,
    parents: Vec<Option<usize>>,
    has_children: Vec<bool>,
    expanded: BTreeSet<usize>,
    revealed: Option<usize>,
}

impl OutlineTree {
    pub fn new(outline: Arc<Outline>) -> Self {
        let entries = &outline.entries;
        let mut parents = Vec::with_capacity(entries.len());
        let mut has_children = vec![false; entries.len()];
        let mut stack: Vec<usize> = Vec::new();
        for entry in entries {
            while stack
                .last()
                .is_some_and(|&open| entries[open].level >= entry.level)
            {
                stack.pop();
            }
            let parent = stack.last().copied();
            if let Some(parent) = parent {
                has_children[parent] = true;
            }
            parents.push(parent);
            stack.push(parents.len() - 1);
        }
        Self {
            outline,
            parents,
            has_children,
            expanded: BTreeSet::new(),
            revealed: None,
        }
    }

    pub fn entries(&self) -> &[OutlineEntry] {
        &self.outline.entries
    }

    pub fn has_children(&self, index: usize) -> bool {
        self.has_children.get(index).copied().unwrap_or(false)
    }

    pub fn is_expanded(&self, index: usize) -> bool {
        self.expanded.contains(&index)
    }

    pub fn current(&self, page: usize) -> Option<usize> {
        self.entries()
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                let target = entry.page?.checked_sub(1)?;
                (target <= page).then_some((target, index))
            })
            .max()
            .map(|(_, index)| index)
    }

    pub fn reveal(&mut self, current: Option<usize>) -> bool {
        if current == self.revealed {
            return false;
        }
        self.revealed = current;
        let mut ancestor = current.and_then(|index| self.parents[index]);
        while let Some(index) = ancestor {
            self.expanded.insert(index);
            ancestor = self.parents[index];
        }
        true
    }

    pub fn toggle(&mut self, index: usize) {
        if !self.expanded.remove(&index) && self.has_children(index) {
            self.expanded.insert(index);
        }
    }

    pub fn visible(&self) -> Vec<usize> {
        let entries = self.entries();
        let mut rows = Vec::new();
        let mut index = 0;
        while index < entries.len() {
            rows.push(index);
            let level = entries[index].level;
            index += 1;
            if !self.expanded.contains(&(index - 1)) {
                while index < entries.len() && entries[index].level > level {
                    index += 1;
                }
            }
        }
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(entries: &[(&str, usize, Option<usize>)]) -> OutlineTree {
        OutlineTree::new(Arc::new(Outline {
            entries: entries
                .iter()
                .map(|(title, level, page)| OutlineEntry {
                    title: (*title).into(),
                    level: *level,
                    page: *page,
                    anchor: None,
                })
                .collect(),
            truncated: false,
        }))
    }

    fn sample() -> OutlineTree {
        tree(&[
            ("Preface", 1, Some(3)),
            ("Part 1", 1, Some(10)),
            ("Chapter 1", 2, Some(10)),
            ("1.1", 3, Some(12)),
            ("1.2", 3, Some(15)),
            ("Chapter 2", 2, Some(20)),
            ("Unlinked", 2, None),
            ("Part 2", 1, Some(40)),
        ])
    }

    #[test]
    fn starts_collapsed_and_expands_the_reading_branch() {
        let mut outline = sample();
        assert_eq!(outline.visible(), [0, 1, 7]);
        assert!(outline.has_children(1) && outline.has_children(2));
        assert!(!outline.has_children(3) && !outline.has_children(7));
        let current = outline.current(15);
        assert_eq!(current, Some(4));
        assert!(outline.reveal(current));
        assert_eq!(outline.visible(), [0, 1, 2, 3, 4, 5, 6, 7]);
        assert!(!outline.reveal(current));
    }

    #[test]
    fn collapsing_hides_descendants_until_the_reading_entry_changes() {
        let mut outline = sample();
        outline.reveal(outline.current(13));
        outline.toggle(1);
        assert_eq!(outline.visible(), [0, 1, 7]);
        outline.reveal(outline.current(13));
        assert_eq!(outline.visible(), [0, 1, 7]);
        outline.reveal(outline.current(15));
        assert_eq!(outline.visible(), [0, 1, 2, 3, 4, 5, 6, 7]);
        outline.toggle(3);
        assert!(!outline.is_expanded(3));
    }

    #[test]
    fn current_entry_is_the_latest_target_at_or_before_the_page() {
        let outline = sample();
        assert_eq!(outline.current(0), None);
        assert_eq!(outline.current(2), Some(0));
        assert_eq!(outline.current(9), Some(2));
        assert_eq!(outline.current(30), Some(5));
        assert_eq!(outline.current(500), Some(7));
    }

    #[test]
    fn levels_that_skip_or_restart_still_nest_correctly() {
        let outline = tree(&[
            ("A", 2, Some(1)),
            ("B", 4, Some(1)),
            ("C", 1, Some(2)),
            ("D", 3, Some(2)),
        ]);
        assert_eq!(outline.parents, [None, Some(0), None, Some(2)]);
    }
}
