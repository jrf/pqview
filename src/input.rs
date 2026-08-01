use unicode_width::UnicodeWidthStr;

pub fn insert(text: &mut String, cursor: &mut usize, character: char) {
    text.insert(*cursor, character);
    *cursor += character.len_utf8();
}

pub fn backspace(text: &mut String, cursor: &mut usize) {
    if let Some(previous) = previous_boundary(text, *cursor) {
        text.drain(previous..*cursor);
        *cursor = previous;
    }
}

pub fn move_left(text: &str, cursor: &mut usize) {
    if let Some(previous) = previous_boundary(text, *cursor) {
        *cursor = previous;
    }
}

pub fn move_right(text: &str, cursor: &mut usize) {
    if *cursor < text.len() {
        *cursor += text[*cursor..].chars().next().map_or(0, char::len_utf8);
    }
}

pub fn display_width(text: &str, cursor: usize) -> u16 {
    text.get(..cursor)
        .unwrap_or(text)
        .width()
        .try_into()
        .unwrap_or(u16::MAX)
}

fn previous_boundary(text: &str, cursor: usize) -> Option<usize> {
    text.get(..cursor)?
        .char_indices()
        .next_back()
        .map(|(index, _)| index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_ascii_and_multibyte_text() {
        let mut text = String::new();
        let mut cursor = 0;

        insert(&mut text, &mut cursor, 'é');
        insert(&mut text, &mut cursor, '界');
        assert_eq!(text, "é界");
        assert_eq!(cursor, text.len());

        move_left(&text, &mut cursor);
        insert(&mut text, &mut cursor, 'a');
        assert_eq!(text, "éa界");

        backspace(&mut text, &mut cursor);
        assert_eq!(text, "é界");
        move_right(&text, &mut cursor);
        assert_eq!(cursor, text.len());
    }

    #[test]
    fn reports_terminal_cell_width() {
        assert_eq!(display_width("é界", "é界".len()), 3);
    }
}
