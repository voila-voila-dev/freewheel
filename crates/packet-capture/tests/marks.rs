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

#[test]
fn reads_the_resources_a_process_used() {
    use packet_capture::ProcessResources;
    let label = "resources sender cpu_ms=770 peak_kb=1576";
    assert_eq!(
        ProcessResources::parse(label, "sender"),
        Some(ProcessResources {
            cpu_ms: 770,
            peak_kb: 1576
        })
    );
    assert_eq!(ProcessResources::parse(label, "receiver"), None);
    assert_eq!(
        ProcessResources::parse("resources sender cpu_ms=7", "sender"),
        None
    );
}
