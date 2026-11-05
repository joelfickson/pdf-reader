use gpui::Keystroke;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum FieldKey {
    Edited,
    Moved,
    Submit,
    Cancel,
    Up,
    Down,
    Ignored,
}

#[derive(Clone, Default, Debug)]
pub(crate) struct TextField {
    pub text: String,
    cursor: usize,
    limit: Option<usize>,
    digits_only: bool,
}

impl TextField {
    pub fn digits(limit: usize) -> Self {
        Self {
            limit: Some(limit),
            digits_only: true,
            ..Self::default()
        }
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
    }

    pub fn before_cursor(&self) -> &str {
        &self.text[..self.byte_index(self.cursor)]
    }

    pub fn after_cursor(&self) -> &str {
        &self.text[self.byte_index(self.cursor)..]
    }

    fn len(&self) -> usize {
        self.text.chars().count()
    }

    fn byte_index(&self, chars: usize) -> usize {
        self.text
            .char_indices()
            .nth(chars)
            .map_or(self.text.len(), |(index, _)| index)
    }

    pub fn insert(&mut self, typed: &str) -> bool {
        let accepted: String = typed
            .chars()
            .filter(|character| !character.is_control())
            .filter(|character| !self.digits_only || character.is_ascii_digit())
            .collect();
        if accepted.is_empty() {
            return false;
        }
        let count = accepted.chars().count();
        if let Some(limit) = self.limit
            && self.len() + count > limit
        {
            return false;
        }
        let at = self.byte_index(self.cursor);
        self.text.insert_str(at, &accepted);
        self.cursor += count;
        true
    }

    pub fn handle(&mut self, keystroke: &Keystroke) -> FieldKey {
        let modifiers = keystroke.modifiers;
        let jump = modifiers.platform || modifiers.alt;
        match keystroke.key.as_str() {
            "enter" => return FieldKey::Submit,
            "escape" => return FieldKey::Cancel,
            "up" => return FieldKey::Up,
            "down" => return FieldKey::Down,
            "left" => {
                self.cursor = if jump {
                    0
                } else {
                    self.cursor.saturating_sub(1)
                };
                return FieldKey::Moved;
            }
            "right" => {
                self.cursor = if jump {
                    self.len()
                } else {
                    (self.cursor + 1).min(self.len())
                };
                return FieldKey::Moved;
            }
            "home" => {
                self.cursor = 0;
                return FieldKey::Moved;
            }
            "end" => {
                self.cursor = self.len();
                return FieldKey::Moved;
            }
            "backspace" => {
                if jump {
                    let at = self.byte_index(self.cursor);
                    self.text.replace_range(..at, "");
                    self.cursor = 0;
                } else if self.cursor > 0 {
                    let start = self.byte_index(self.cursor - 1);
                    let end = self.byte_index(self.cursor);
                    self.text.replace_range(start..end, "");
                    self.cursor -= 1;
                }
                return FieldKey::Edited;
            }
            "delete" => {
                if self.cursor < self.len() {
                    let start = self.byte_index(self.cursor);
                    let end = self.byte_index(self.cursor + 1);
                    self.text.replace_range(start..end, "");
                }
                return FieldKey::Edited;
            }
            _ => {}
        }
        if modifiers.platform || modifiers.control {
            return FieldKey::Ignored;
        }
        match keystroke.key_char.as_deref() {
            Some(typed) if self.insert(typed) => FieldKey::Edited,
            _ => FieldKey::Ignored,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(source: &str) -> Keystroke {
        Keystroke::parse(source).unwrap()
    }

    #[test]
    fn typing_editing_and_control_keys() {
        let mut field = TextField::default();
        assert_eq!(field.handle(&key("a->a")), FieldKey::Edited);
        assert_eq!(field.handle(&key("shift-b->B")), FieldKey::Edited);
        assert_eq!(field.handle(&key("space-> ")), FieldKey::Edited);
        assert_eq!(field.text, "aB ");
        assert_eq!(field.handle(&key("backspace")), FieldKey::Edited);
        assert_eq!(field.text, "aB");
        assert_eq!(field.handle(&key("cmd-c")), FieldKey::Ignored);
        assert_eq!(field.handle(&key("enter")), FieldKey::Submit);
        assert_eq!(field.handle(&key("escape")), FieldKey::Cancel);
        assert_eq!(field.handle(&key("down")), FieldKey::Down);
    }

    #[test]
    fn cursor_moves_and_edits_in_place() {
        let mut field = TextField::default();
        field.insert("café");
        field.handle(&key("left"));
        field.handle(&key("left"));
        assert_eq!(field.before_cursor(), "ca");
        field.insert("r");
        assert_eq!(field.text, "carfé");
        field.handle(&key("delete"));
        assert_eq!(field.text, "caré");
        field.handle(&key("backspace"));
        assert_eq!(field.text, "caé");
        field.handle(&key("cmd-left"));
        assert_eq!(field.after_cursor(), "caé");
        field.handle(&key("end"));
        assert_eq!(field.before_cursor(), "caé");
        field.handle(&key("cmd-backspace"));
        assert!(field.text.is_empty());
    }

    #[test]
    fn digit_fields_reject_letters_and_respect_limit() {
        let mut field = TextField::digits(3);
        field.handle(&key("1->1"));
        field.handle(&key("x->x"));
        field.handle(&key("2->2"));
        field.handle(&key("3->3"));
        assert_eq!(field.handle(&key("4->4")), FieldKey::Ignored);
        assert_eq!(field.text, "123");
        assert!(!field.insert("4"));
    }
}
