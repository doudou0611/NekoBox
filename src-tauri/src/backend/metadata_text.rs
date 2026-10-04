//! Keep source-maintained Chinese prose separate from untranslated summaries.

fn is_han(character: char) -> bool {
    ('\u{3400}'..='\u{4dbf}').contains(&character) || ('\u{4e00}'..='\u{9fff}').contains(&character)
}

fn is_kana(character: char) -> bool {
    character != '\u{30fb}'
        && (('\u{3040}'..='\u{30ff}').contains(&character)
            || ('\u{ff66}'..='\u{ff9f}').contains(&character))
}

/// A short, balanced kana reading immediately after a Han name is an annotation,
/// not the language of the surrounding prose (e.g. 赞咲良（さんさら）).
fn reading_length(value: &str, closing: char) -> Option<usize> {
    for (length, (index, character)) in value.char_indices().skip(1).enumerate() {
        if character == closing {
            return (length > 0).then_some(index + character.len_utf8());
        }
        if !is_kana(character) || length == 16 {
            return None;
        }
    }
    None
}

pub(crate) fn is_chinese_description(value: &str) -> bool {
    let mut chinese = 0;
    let mut other_letters = 0;
    let mut previous = None;
    let mut skip_until = 0;
    for (index, character) in value.char_indices() {
        if index < skip_until {
            continue;
        }
        if previous.is_some_and(is_han) && matches!(character, '(' | '（') {
            let closing = if character == '(' { ')' } else { '）' };
            if let Some(length) = reading_length(&value[index..], closing) {
                skip_until = index + length;
                previous = None;
                continue;
            }
        }
        previous = Some(character);
        if is_kana(character) {
            return false;
        }
        if is_han(character) {
            chinese += 1;
        } else if character.is_alphabetic() {
            other_letters += 1;
        }
    }
    chinese > 0 && chinese >= other_letters
}

/// Long Latin fragments in otherwise Chinese prose can contain untranslated names.
/// Prefer a complete Chinese source before requesting optional translation.
pub(crate) fn is_complete_chinese_description(value: &str) -> bool {
    is_chinese_description(value) && value.chars().filter(char::is_ascii_alphabetic).count() <= 8
}

pub(super) fn description_fields(value: Option<String>) -> Vec<(String, String)> {
    let Some(value) = value.filter(|value| !value.trim().is_empty()) else {
        return Vec::new();
    };
    let value = value.replace("\r\n", "\n").trim().to_owned();
    let chinese = value
        .lines()
        .find_map(|line| {
            ["中文介绍", "中文简介"]
                .into_iter()
                .find_map(|marker| {
                    let suffix = line.trim().strip_prefix(marker)?;
                    suffix.strip_prefix(['：', ':'])
                })
                .map(|_| line)
        })
        .and_then(|line| value.find(line).map(|index| value[index..].trim()))
        .filter(|text| is_chinese_description(text))
        .or_else(|| is_chinese_description(&value).then_some(value.as_str()));
    let mut fields = Vec::new();
    if let Some(chinese) = chinese {
        fields.push(("description_zh".into(), chinese.to_owned()));
    }
    fields.push(("description".into(), value));
    fields
}

pub(super) fn has_chinese_description(value: Option<&str>) -> Option<bool> {
    value.filter(|value| !value.trim().is_empty()).map(|value| {
        description_fields(Some(value.into()))
            .iter()
            .any(|(field, _)| field == "description_zh")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_name_readings_in_chinese_but_rejects_japanese_prose() {
        for chinese in [
            "主角・莲佛雪之进一直以来都专心于武道的修行。",
            "私立赞咲良（さんさら）学园的调查。",
            "私立赞咲良(さんさら)学园的调查。",
            "私立赞咲良（ｻﾝｻﾗ）学园的调查。",
        ] {
            assert!(is_chinese_description(chinese), "{chinese}");
            assert!(is_complete_chinese_description(chinese));
            assert_eq!(
                description_fields(Some(chinese.into()))[0].0,
                "description_zh"
            );
        }
        for original in [
            "私立讃咲良（さんさら）学園での調査。",
            "私立赞咲良（さんさら）学园，彼女は同級生。",
            "私立赞咲良（さんさら学园的调查。",
            "私立赞咲良（さんさら)学园的调查。",
            "私立赞咲良（さんさらです。）学园的调查。",
            "主人公（彼女は学生）展开调查。",
            "（さんさら）",
            "（さんさら）学园的调查。",
            "学园（あああああああああああああああああ）的调查。",
        ] {
            assert!(!is_chinese_description(original), "{original}");
        }
    }

    #[test]
    fn keeps_chinese_names_from_the_source_and_rejects_japanese_and_english() {
        let chinese = "女仆小春来到宿舍。柳诗音和柳花音迎接主人公。";
        assert_eq!(
            description_fields(Some(chinese.into())),
            vec![
                ("description_zh".into(), chinese.into()),
                ("description".into(), chinese.into())
            ]
        );
        for original in [
            "彼女は主人公の同級生です。",
            "A maid named Scarlet arrived.",
        ] {
            assert_eq!(
                description_fields(Some(original.into())),
                vec![("description".into(), original.into())]
            );
        }
        assert!(!is_chinese_description("The Chinese title is 中文名."));
        assert!(description_fields(Some("  ".into())).is_empty());
    }

    #[test]
    fn uses_the_chinese_section_of_a_bilingual_source() {
        let fields = description_fields(Some(
            "主人公は高校生です。\r\n\r\n中文介绍：\r\n主人公与小春相遇。".into(),
        ));
        assert_eq!(fields[0].0, "description_zh");
        assert_eq!(fields[0].1, "中文介绍：\n主人公与小春相遇。");
        assert!(fields[1].1.contains("高校生"));
        assert_eq!(has_chinese_description(Some("中文简介。")), Some(true));
        assert_eq!(
            has_chinese_description(Some("主人公は高校生。")),
            Some(false)
        );
        assert_eq!(has_chinese_description(Some("  ")), None);
        assert_eq!(has_chinese_description(None), None);
    }
}
