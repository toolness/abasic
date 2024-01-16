use abasic_core::SourceFileAnalyzer;

#[test]
fn it_works() {
    let program = r#"10 print "hi"
    20 for i = 1 to 3:next i
    "#;

    let lines = program
        .split("\n")
        .map(|line| line.trim_start())
        .map(|s| s.to_owned());
    let mut analzyer = SourceFileAnalyzer::analyze(lines.clone().collect::<Vec<_>>().join("\n"));
    let analyzer_messages = analzyer.take_messages();
    if analyzer_messages.len() != 0 {
        panic!("Expected analyzer for program {program} to be empty but got {analyzer_messages:?}");
    }
}
