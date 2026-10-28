use super::*;
use crate::probe::TranscriptLanguages;

fn video(title: &str) -> Video {
    Video {
        id: "jNQXAC9IVRw".into(),
        url: "https://www.youtube.com/watch?v=jNQXAC9IVRw".into(),
        title: title.into(),
        channel: Some("jawed".into()),
        upload_date: Some("20050424".into()),
        thumbnail: None,
        duration: Some(19.0),
        formats: Vec::new(),
        transcripts: TranscriptLanguages::default(),
        original_language: None,
        chapters: Vec::new(),
        was_live: false,
    }
}

#[test]
fn the_default_template_names_the_file_by_the_title() {
    assert_eq!(
        file_name(DEFAULT_TEMPLATE, &video("Me at the zoo")),
        "Me at the zoo"
    );
}

#[test]
fn a_template_puts_in_the_title_channel_and_date() {
    assert_eq!(
        file_name("{date} {channel} - {title}", &video("Me at the zoo")),
        "2005-04-24 jawed - Me at the zoo"
    );
}

#[test]
fn a_template_keeps_what_isnt_a_variable() {
    assert_eq!(
        file_name("{channel} {name} {title}", &video("Me at the zoo")),
        "jawed {name} Me at the zoo"
    );
}

#[test]
fn characters_windows_forbids_are_replaced_by_lookalikes() {
    assert_eq!(
        file_name(
            DEFAULT_TEMPLATE,
            &video("Why? A/B: \"test\" | <1> * \\ end. ")
        ),
        "Why？ A⧸B： ＂test＂ ｜ ＜1＞ ＊ ⧹ end"
    );
}

#[test]
fn characters_windows_forbids_in_the_template_are_replaced_too() {
    assert_eq!(
        file_name("{channel}/{title}?", &video("Me at the zoo")),
        "jawed⧸Me at the zoo？"
    );
}

#[test]
fn a_missing_channel_or_date_leaves_no_separator_at_the_edges() {
    let without = Video {
        channel: None,
        upload_date: None,
        ..video("Me at the zoo")
    };

    assert_eq!(file_name("{channel} - {title}", &without), "Me at the zoo");
    assert_eq!(file_name("{title}, {date}", &without), "Me at the zoo");
    assert_eq!(
        file_name("{title} - {date} - {channel}", &without),
        "Me at the zoo"
    );
    assert_eq!(
        file_name(
            "{date} {channel} - {title}",
            &Video {
                upload_date: Some("20050424".into()),
                ..without
            }
        ),
        "2005-04-24 - Me at the zoo"
    );
}

#[test]
fn a_missing_variable_leaves_the_titles_own_separators() {
    let without_channel = |title: &str| Video {
        channel: None,
        ..video(title)
    };

    assert_eq!(
        file_name("{channel} - {title}", &without_channel(".NET tips")),
        ".NET tips"
    );
    assert_eq!(
        file_name("{title} - {channel}", &without_channel("Part 1 -")),
        "Part 1 -"
    );
}

#[test]
fn a_long_name_is_cut() {
    let name = file_name("{title} - {channel}", &video(&"a".repeat(200)));

    assert_eq!(name, "a".repeat(150));
}

#[test]
fn a_name_that_cant_be_a_file_name_gives_way_to_the_video_id() {
    for (template, title) in [
        (DEFAULT_TEMPLATE, ""),
        (DEFAULT_TEMPLATE, " . "),
        (DEFAULT_TEMPLATE, "CON"),
        (DEFAULT_TEMPLATE, "nul"),
        ("{title}.txt", "aux"),
        ("", "Me at the zoo"),
    ] {
        assert_eq!(
            file_name(template, &video(title)),
            "jNQXAC9IVRw",
            "{template:?} {title:?}"
        );
    }
}
