//! Reading the lab's marks file.

use packet_capture::Mark;

#[test]
fn marks_are_a_timestamp_then_a_label() {
    let marks = Mark::parse_all(
        "1794330577670878 cut link 2 both\ngarbage\n1794330587670878 restore link 2 both\n",
    );
    assert_eq!(marks.len(), 2);
    assert_eq!(marks[0].at_us, 1_794_330_577_670_878);
    assert_eq!(marks[1].label, "restore link 2 both");
}
