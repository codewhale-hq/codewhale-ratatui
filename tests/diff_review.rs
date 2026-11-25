use codewhale_ratatui::{
    DiffReviewFile, DiffReviewHeader, DiffScope, Paint, next_unviewed,
    testing::{Profile, render, text},
};

#[test]
fn next_unviewed_wraps_skips_viewed_and_never_returns_sole_current() {
    let empty: [DiffReviewFile<'_>; 0] = [];
    assert_eq!(next_unviewed(&empty, None), None);

    let files = [
        DiffReviewFile::new("a.rs", 4, 1),
        DiffReviewFile::new("b.rs", 2, 0).viewed(true),
        DiffReviewFile::new("c.rs", 8, 3),
    ];
    assert_eq!(next_unviewed(&files, None), Some(0));
    assert_eq!(next_unviewed(&files, Some(99)), Some(0));
    assert_eq!(next_unviewed(&files, Some(0)), Some(2));
    assert_eq!(next_unviewed(&files, Some(2)), Some(0));

    let only_current = [
        DiffReviewFile::new("a.rs", 4, 1).viewed(true),
        DiffReviewFile::new("b.rs", 2, 0).viewed(true),
        DiffReviewFile::new("c.rs", 8, 3),
    ];
    assert_eq!(next_unviewed(&only_current, Some(2)), None);
    assert_eq!(next_unviewed(&only_current, None), Some(2));

    let all_viewed = [
        DiffReviewFile::new("a.rs", 4, 1).viewed(true),
        DiffReviewFile::new("b.rs", 2, 0).viewed(true),
    ];
    assert_eq!(next_unviewed(&all_viewed, Some(0)), None);
    assert_eq!(next_unviewed(&all_viewed, None), None);
}

#[test]
fn diff_review_header_paints_scope_viewed_counts_and_ascii_fallback() {
    let files = [
        DiffReviewFile::new("a.rs", 12, 3).viewed(true),
        DiffReviewFile::new("b.rs", 5, 2),
    ];
    let header = DiffReviewHeader::from_files(DiffScope::Branch, &files);
    assert_eq!(header.viewed, 1);
    assert_eq!(header.total, 2);
    assert_eq!(header.added, 17);
    assert_eq!(header.removed, 5);

    for profile in Profile::ALL {
        let theme = profile.theme();
        for width in [0u16, 1, 10, 24, 48] {
            let rendered = render(width, 1, |area, buf| header.paint(area, buf, &theme));
            let shown = text(&rendered);
            if width >= 30 {
                assert!(shown.contains("Branch"), "{}: {shown}", profile.name());
                assert!(shown.contains("1/2"), "{}: {shown}", profile.name());
                assert!(shown.contains("+17"), "{}: {shown}", profile.name());
                if theme.ascii() {
                    assert!(shown.contains("-5"), "{}: {shown}", profile.name());
                    assert!(shown.is_ascii(), "non-ASCII: {shown}");
                } else {
                    assert!(shown.contains("\u{2212}5"), "{}: {shown}", profile.name());
                }
            }
        }
    }
}
