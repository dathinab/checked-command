#![cfg(unix)]
//! Integration tests for the redirect functionality

use mapped_command::{pipe::Redirect, prelude::*};
use std::os::unix::io::{FromRawFd, IntoRawFd};
use std::{fs, path::PathBuf};

#[test]
fn redirect_into_file() {
    let mut path = PathBuf::from(file!());
    path.set_extension("test_file");
    let file = fs::File::create(&path).unwrap();

    Command::new("bash", ReturnNothing)
        .with_arguments(&["-c", "echo abcde"])
        //TODO change API to accept (Redirect::from(file))
        .with_custom_stdout_setup(Redirect::from(file))
        .run()
        .unwrap();

    let data = fs::read_to_string(&path).unwrap();
    assert_eq!(data, "abcde\n");

    fs::remove_file(path).unwrap();
}

#[test]
fn redirect_from_raw_fd() {
    let mut path = PathBuf::from(file!());
    path.set_extension("test_file2");
    let file = fs::File::create(&path).unwrap();
    let redirect = unsafe { Redirect::from_raw_fd(file.into_raw_fd()) };

    Command::new("bash", ReturnNothing)
        .with_arguments(&["-c", "echo abcde"])
        .with_custom_stdout_setup(redirect)
        .run()
        .unwrap();

    let data = fs::read_to_string(&path).unwrap();
    assert_eq!(data, "abcde\n");

    fs::remove_file(path).unwrap();
}

mod connect_input_and_output_of_two_sub_processes {
    use std::{convert::TryFrom, io::Write};

    use super::*;

    #[test]
    fn out_to_in() {
        let mut child = Command::new("bash", ReturnNothing)
            .with_arguments(&[ "-c", r#"echo SETUP1 >&2; sleep 0.1; read line; echo READY1 >&2; echo "<$line>"; echo DONE1 >&2"#])
            .with_custom_stdin_setup(PipeSetup::Piped)
            .with_custom_stdout_setup(PipeSetup::Piped)
            .spawn()
            .unwrap();

        //FIXME This has a bit to much "failable" parts for my opinion given that it normally should not fail
        //      I think I will remove the "try_" part in "try_from_raw_fd", as there *always* should be an underlying
        //      raw fd outside of mocking. Where we can panic.
        let child_output = child.take_stdout().unwrap();
        let child_output = Redirect::try_from(child_output).unwrap();
        let mut child_input = child.take_stdin().unwrap();

        let other_child = Command::new("bash", ReturnStdoutString)
            .with_arguments(&[
                "-c",
                r#"echo SETUP2 >&2; read line;  echo READY2 >&2; echo ".$line."; echo DONE2 >&2"#,
            ])
            .with_custom_stdin_setup(child_output)
            .spawn()
            .unwrap();

        child_input.write_all(b"hyhohe\n").unwrap();
        child_input.flush().unwrap();

        let received = other_child.wait().unwrap();
        let () = child.wait().unwrap();

        assert_eq!(received, ".<hyhohe>.\n")
    }

    #[test]
    fn in_to_out() {
        let mut child = Command::new("bash", ReturnStdoutString)
            .with_arguments(&[
                "-c",
                r#"echo SETUP2 >&2; read line;  echo READY2 >&2; echo ".$line."; echo DONE2 >&2"#,
            ])
            .with_custom_stdin_setup(PipeSetup::Piped)
            .spawn()
            .unwrap();

        let child_input = child.take_stdin().unwrap();
        let child_input = Redirect::try_from(child_input).unwrap();

        let mut other_child = Command::new("bash", ReturnNothing)
            .with_arguments(&[ "-c", r#"echo SETUP1 >&2; sleep 0.1; read line; echo READY1 >&2; echo "<$line>"; echo DONE1 >&2"#])
            .with_custom_stdout_setup(child_input)
            .with_custom_stdin_setup(PipeSetup::Piped)
            .spawn()
            .unwrap();

        let mut child_input = other_child.take_stdin().unwrap();

        child_input.write_all(b"hyhohe\n").unwrap();
        child_input.flush().unwrap();

        let received = child.wait().unwrap();
        let () = other_child.wait().unwrap();

        assert_eq!(received, ".<hyhohe>.\n")
    }
}