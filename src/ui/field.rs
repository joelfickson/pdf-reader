use gpui::Keystroke;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum FieldKey {
    Edited,
    Submit,
    Cancel,
    Up,
    Down,
    Ignored,
}

#[derive(Clone, Default, Debug)]
pub(crate) struct TextField {
    pub text: String,
    limit: Option<usize>,
    digits_only: bool,
}

impl TextField {
    pub fn digits(limit: usize) -> Self {
        Self {
            text: String::new(),
            limit: Some(limit),
            digits_only: true,
        }
    }

    pub fn clear(&mut self) {
        self.text.clear();
    }

    pub fn handle(&mut self, keystroke: &Keystroke) -> FieldKey {
        let modifiers = keystroke.modifiers;
        if modifiers.platform || modifiers.control {
            return FieldKey::Ignored;
        }
        match keystroke.key.as_str() {
            "enter" => return FieldKey::Submit,
            "escape" => return FieldKey::Cancel,
            "up" => return FieldKey::Up,
            "down" => return FieldKey::Down,
            "backspace" => {
                self.text.pop();
                return FieldKey::Edited;
            }
            _ => {}
        }
        let Some(typed) = keystroke.key_char.as_deref() else {
            return FieldKey::Ignored;
        };
        let accepted: String = typed
            .chars()
            .filter(|character| !character.is_control())
            .filter(|character| !self.digits_only || character.is_ascii_digit())
            .collect();
        if accepted.is_empty() {
            return FieldKey::Ignored;
        }
        if let Some(limit) = self.limit
            && self.text.chars().count() + accepted.chars().count() > limit
        {
            return FieldKey::Ignored;
        }
        self.text.push_str(&accepted);
        FieldKey::Edited
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
    fn digit_fields_reject_letters_and_respect_limit() {
        let mut field = TextField::digits(3);
        field.handle(&key("1->1"));
        field.handle(&key("x->x"));
        field.handle(&key("2->2"));
        field.handle(&key("3->3"));
        assert_eq!(field.handle(&key("4->4")), FieldKey::Ignored);
        assert_eq!(field.text, "123");
    }
}
