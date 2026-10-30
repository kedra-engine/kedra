//! No GUI automation: test exact platform dispatch and real process error paths.
use super::*;

#[test]
fn platform_dispatch_preserves_argument_boundaries_and_unicode() {
    let arguments = vec![
        OsString::from("name Юникод with spaces"),
        "--flag=value".into(),
        "".into(),
    ];
    let root = Path::new("/output/Independent Probe.app");
    let binary = Path::new("/output/probe");
    let mac = command(NativeTarget::MacIntel, root, binary, &arguments);
    assert_eq!(mac.get_program(), "/usr/bin/open");
    let args: Vec<_> = mac.get_args().map(|v| v.to_os_string()).collect();
    assert_eq!(
        args,
        vec![
            "-n".into(),
            root.as_os_str().to_owned(),
            "--args".into(),
            "name Юникод with spaces".into(),
            "--flag=value".into(),
            "".into()
        ]
    );
    for target in [NativeTarget::Windows, NativeTarget::Linux] {
        let direct = command(target, root, binary, &arguments);
        assert_eq!(direct.get_program(), binary);
        assert_eq!(
            direct.get_args().collect::<Vec<_>>(),
            arguments.iter().map(|v| v.as_os_str()).collect::<Vec<_>>()
        );
    }
}

#[test]
fn missing_application_cannot_be_dispatched_and_does_not_discard_result()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = crate::test_support::configuration()?;
    let result = crate::application::layout(&fixture.app);
    crate::application::publish(fixture.root.path(), &result)?;
    let built = BuiltApplication {
        output: fixture.root.path().into(),
        result,
    };
    assert!(request(&fixture.app, &built, &[]).is_err());
    assert!(fixture.root.path().join("build-result.json").is_file());
    Ok(())
}
