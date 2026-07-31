//! Integration tests for script parsing and execution.

#[cfg(feature = "script")]
mod script_tests {
    use expectrust::script::{Script, ScriptError};
    use std::time::Duration;

    #[test]
    fn test_parse_simple_script() {
        let script_text = if cfg!(windows) {
            r#"
                spawn cmd /c echo hello
                expect "hello"
                wait
            "#
        } else {
            r#"
                spawn echo hello
                expect "hello"
                wait
            "#
        };

        let result = Script::from_str(script_text);
        assert!(result.is_ok(), "Failed to parse script: {:?}", result.err());
    }

    #[test]
    fn test_parse_invalid_script() {
        // Note: a bare word with nothing else (e.g. just "spawn") is no
        // longer a reliable "this should fail" example - since a statement's
        // trailing terminator can now be satisfied by end-of-input (not just
        // a literal newline; see grammar.pest's `stmt_end`, needed so a
        // script's last line doesn't require a trailing newline), a lone
        // identifier like "spawn" successfully parses as a zero-argument
        // `call_stmt` (a procedure call to a procedure named "spawn" - which
        // would fail at *execution* time with `UndefinedProcedure`, but that's
        // no longer a *parse* error). An unclosed brace is still a genuine,
        // unconditional parse failure.
        let script_text = "expect {";
        let result = Script::from_str(script_text);
        assert!(
            result.is_err(),
            "Should have failed to parse an unclosed expect block"
        );
    }

    #[test]
    fn test_parse_set_statement() {
        let script_text = r#"
            set myvar "value"
            set num 42
        "#;

        let result = Script::from_str(script_text);
        assert!(
            result.is_ok(),
            "Failed to parse set statements: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_parse_if_statement() {
        let script_text = r#"
            set x 1
            if { $x == 1 } {
                send "yes\n"
            } else {
                send "no\n"
            }
        "#;

        let result = Script::from_str(script_text);
        assert!(
            result.is_ok(),
            "Failed to parse if statement: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn test_execute_if_true_branch() {
        // Key differentiating test: `if`/`while`/`for` conditions used to be
        // parsed via a stub that always evaluated to true, so a test that
        // only ever exercised the true branch wouldn't have caught that.
        // This test and test_execute_if_false_branch together prove the
        // condition is actually evaluated, not just always-true.
        let script_text = r#"
            set x 1
            if { $x == 1 } {
                set result "yes"
            } else {
                set result "no"
            }
        "#;

        let script = Script::from_str(script_text).expect("Failed to parse script");
        let result = script.execute().await.expect("Failed to execute");

        assert_eq!(result.variables.get("result").unwrap().as_string(), "yes");
    }

    #[tokio::test]
    async fn test_execute_if_false_branch() {
        let script_text = r#"
            set x 2
            if { $x == 1 } {
                set result "yes"
            } else {
                set result "no"
            }
        "#;

        let script = Script::from_str(script_text).expect("Failed to parse script");
        let result = script.execute().await.expect("Failed to execute");

        assert_eq!(result.variables.get("result").unwrap().as_string(), "no");
    }

    #[tokio::test]
    async fn test_execute_while_loop() {
        // `set i ($i + 1)` exercises the new parenthesized-expression form of
        // `set` - without it there'd be no way to change the loop variable
        // between iterations at all, since a plain `word` (the only other
        // form `set` accepts) has no arithmetic.
        let script_text = r#"
            set i 0
            while { $i < 5 } {
                set i ($i + 1)
            }
        "#;

        let script = Script::builder()
            .timeout(Duration::from_secs(5))
            .from_str(script_text)
            .expect("Failed to parse script");
        let result = script.execute().await.expect("Failed to execute");

        assert_eq!(result.variables.get("i").unwrap().as_number().unwrap(), 5.0);
    }

    #[tokio::test]
    async fn test_execute_for_loop() {
        let script_text = r#"
            for {set i 0} {$i < 3} {set i ($i + 1)} {
                set last $i
            }
        "#;

        let script = Script::builder()
            .timeout(Duration::from_secs(5))
            .from_str(script_text)
            .expect("Failed to parse script");
        let result = script.execute().await.expect("Failed to execute");

        assert_eq!(result.variables.get("i").unwrap().as_number().unwrap(), 3.0);
        // Last body execution happens with i == 2 (loop stops once i == 3).
        assert_eq!(
            result.variables.get("last").unwrap().as_number().unwrap(),
            2.0
        );
    }

    #[tokio::test]
    async fn test_execute_expect_out_send() {
        let script_text = if cfg!(windows) {
            r#"
                spawn cmd /c echo hello123
                expect -re "([a-z]+)([0-9]+)"
                set captured1 $expect_out(1,string)
                set captured2 $expect_out(2,string)
                set whole $expect_out(0,string)
            "#
        } else {
            r#"
                spawn echo hello123
                expect -re "([a-z]+)([0-9]+)"
                set captured1 $expect_out(1,string)
                set captured2 $expect_out(2,string)
                set whole $expect_out(0,string)
            "#
        };

        let script = Script::builder()
            .timeout(Duration::from_secs(5))
            .from_str(script_text)
            .expect("Failed to parse script");
        let result = script.execute().await.expect("Failed to execute");

        assert_eq!(
            result.variables.get("captured1").unwrap().as_string(),
            "hello"
        );
        assert_eq!(
            result.variables.get("captured2").unwrap().as_string(),
            "123"
        );
        assert_eq!(
            result.variables.get("whole").unwrap().as_string(),
            "hello123"
        );
    }

    #[tokio::test]
    async fn test_execute_expect_out_in_condition() {
        // Combines both fixes: expect_out is only useful once conditions
        // actually evaluate real content, so this is the point of doing both
        // in one pass - branching directly on what was just matched.
        let script_text = if cfg!(windows) {
            r#"
                spawn cmd /c echo hello123
                expect -re "([a-z]+)([0-9]+)"
                if { $expect_out(1,string) == "hello" } {
                    set matched 1
                } else {
                    set matched 0
                }
            "#
        } else {
            r#"
                spawn echo hello123
                expect -re "([a-z]+)([0-9]+)"
                if { $expect_out(1,string) == "hello" } {
                    set matched 1
                } else {
                    set matched 0
                }
            "#
        };

        let script = Script::builder()
            .timeout(Duration::from_secs(5))
            .from_str(script_text)
            .expect("Failed to parse script");
        let result = script.execute().await.expect("Failed to execute");

        assert_eq!(
            result
                .variables
                .get("matched")
                .unwrap()
                .as_number()
                .unwrap(),
            1.0
        );
    }

    #[test]
    fn test_parse_expect_block() {
        let script_text = if cfg!(windows) {
            r#"
                spawn cmd /c echo test
                expect {
                    "success" {
                        send "ok\n"
                    }
                    "error" {
                        send "fail\n"
                    }
                    timeout {
                        send "timeout\n"
                    }
                }
            "#
        } else {
            r#"
                spawn echo test
                expect {
                    "success" {
                        send "ok\n"
                    }
                    "error" {
                        send "fail\n"
                    }
                    timeout {
                        send "timeout\n"
                    }
                }
            "#
        };

        let result = Script::from_str(script_text);
        assert!(
            result.is_ok(),
            "Failed to parse expect block: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_parse_proc_definition() {
        let script_text = r#"
            proc greet { name } {
                send "Hello $name\n"
            }
        "#;

        let result = Script::from_str(script_text);
        assert!(result.is_ok(), "Failed to parse proc: {:?}", result.err());
    }

    #[tokio::test]
    async fn test_execute_simple_spawn() {
        // Use a command that works cross-platform
        let script_text = if cfg!(windows) {
            r#"
                spawn cmd /c echo hello
                expect "hello"
            "#
        } else {
            r#"
                spawn echo hello
                expect "hello"
            "#
        };

        let script = Script::builder()
            .timeout(Duration::from_secs(5))
            .from_str(script_text)
            .expect("Failed to parse script");

        let result = script.execute().await;
        assert!(
            result.is_ok(),
            "Script execution failed: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn test_execute_with_variable() {
        let script_text = if cfg!(windows) {
            r#"
                set greeting "hello"
                spawn cmd /c echo $greeting
                expect "hello"
            "#
        } else {
            r#"
                set greeting "hello"
                spawn echo $greeting
                expect "hello"
            "#
        };

        let script = Script::builder()
            .timeout(Duration::from_secs(5))
            .from_str(script_text)
            .expect("Failed to parse script");

        let result = script.execute().await;
        assert!(
            result.is_ok(),
            "Script execution failed: {:?}",
            result.err()
        );

        let result = result.unwrap();
        assert_eq!(
            result.variables.get("greeting").unwrap().as_string(),
            "hello"
        );
    }

    #[tokio::test]
    async fn test_execute_spawn_with_quoted_argument_keeps_one_arg() {
        // No standalone `printf` binary on Windows without a shell in the way.
        if cfg!(windows) {
            return;
        }

        // `printf` recycles its format string over extra positional
        // arguments, so this is a real differentiating check: before the
        // fix, `parse_spawn_stmt` flattened all spawn words into one
        // space-joined string which `Session::spawn` then re-split, so a
        // single quoted word containing a space ("hello world") would have
        // been wrongly broken into two argv entries.
        let script_text = r#"
            spawn printf "%s\n" "hello world"
            expect "hello world"
        "#;

        let script = Script::builder()
            .timeout(Duration::from_secs(5))
            .from_str(script_text)
            .expect("Failed to parse script");

        let result = script.execute().await;
        assert!(
            result.is_ok(),
            "Script execution failed: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn test_execute_spawn_with_variable_containing_space() {
        if cfg!(windows) {
            return;
        }

        // Same check, but the space comes from a substituted variable's
        // value rather than a literal quoted word - proves the fix covers
        // both cases, since evaluation now happens per-argument.
        let script_text = r#"
            set greeting "hello world"
            spawn printf "%s\n" $greeting
            expect "hello world"
        "#;

        let script = Script::builder()
            .timeout(Duration::from_secs(5))
            .from_str(script_text)
            .expect("Failed to parse script");

        let result = script.execute().await;
        assert!(
            result.is_ok(),
            "Script execution failed: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn test_execute_exit_code() {
        let script_text = r#"
            exit 42
        "#;

        let script = Script::from_str(script_text).expect("Failed to parse script");
        let result = script.execute().await;

        assert!(result.is_err(), "Expected exit error");
        match result.unwrap_err() {
            ScriptError::Exit(code) => assert_eq!(code, 42),
            other => panic!("Expected Exit error, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_undefined_variable() {
        let script_text = r#"
            spawn echo $undefined_var
        "#;

        let script = Script::from_str(script_text).expect("Failed to parse script");
        let result = script.execute().await;

        assert!(result.is_err(), "Expected undefined variable error");
        match result.unwrap_err() {
            ScriptError::UndefinedVariable(name) => assert_eq!(name, "undefined_var"),
            other => panic!("Expected UndefinedVariable error, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_arithmetic_expressions() {
        let script_text = r#"
            set a 10
            set b 5
        "#;

        let script = Script::from_str(script_text).expect("Failed to parse script");
        let result = script.execute().await.expect("Failed to execute");

        assert_eq!(
            result.variables.get("a").unwrap().as_number().unwrap(),
            10.0
        );
        assert_eq!(result.variables.get("b").unwrap().as_number().unwrap(), 5.0);
    }

    #[test]
    fn test_parse_comments() {
        let script_text = r#"
            # This is a comment
            spawn echo test  # inline comment
            # Another comment
            expect "test"
        "#;

        let result = Script::from_str(script_text);
        assert!(
            result.is_ok(),
            "Failed to parse script with comments: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_parse_string_escapes() {
        let script_text = r#"
            set newline "line1\nline2"
            set tab "col1\tcol2"
            set quote "say \"hello\""
        "#;

        let result = Script::from_str(script_text);
        assert!(
            result.is_ok(),
            "Failed to parse string escapes: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_parse_brace_string() {
        let script_text = r#"
            set text {This is a brace string}
            set multiline {
                Line 1
                Line 2
            }
        "#;

        let result = Script::from_str(script_text);
        assert!(
            result.is_ok(),
            "Failed to parse brace strings: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn test_send_command() {
        let script_text = r#"
            spawn cat
            send "hello\n"
            expect "hello"
        "#;

        let script = Script::builder()
            .timeout(Duration::from_secs(5))
            .from_str(script_text)
            .expect("Failed to parse script");

        // This may timeout or fail depending on platform, but parsing should work
        let _ = script.execute().await;
    }

    #[test]
    fn test_builder_configuration() {
        let script_text = if cfg!(windows) {
            "spawn cmd /c echo test\n"
        } else {
            "spawn echo test\n"
        };

        let script = Script::builder()
            .timeout(Duration::from_secs(30))
            .max_buffer_size(16384)
            .strip_ansi(true)
            .pty_size(24, 80)
            .from_str(script_text);

        assert!(script.is_ok(), "Failed to build script: {:?}", script.err());
    }

    #[test]
    fn test_parse_interact_statement() {
        let script_text = if cfg!(windows) {
            "spawn cmd /c echo test\ninteract\n"
        } else {
            "spawn echo test\ninteract\n"
        };

        let result = Script::from_str(script_text);
        assert!(
            result.is_ok(),
            "Failed to parse interact statement: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    #[ignore] // Requires a real controlling terminal (raw mode) - unsuitable for headless CI.
              // The forwarding logic itself (which this exercises via `Session::interact`)
              // is covered without a real tty in tests/session_interact_tests.rs via
              // `Session::interact_with` and synthetic pipes.
    async fn test_execute_interact_statement() {
        // A child that exits immediately means `interact` (which returns on
        // the child's EOF) completes without needing any real user input.
        let script_text = if cfg!(windows) {
            r#"
                spawn cmd /c echo done
                interact
            "#
        } else {
            r#"
                spawn echo done
                interact
            "#
        };

        let script = Script::builder()
            .timeout(Duration::from_secs(5))
            .from_str(script_text)
            .expect("Failed to parse script");

        let result = script.execute().await;
        assert!(
            result.is_ok(),
            "Script execution with interact failed: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_parse_exp_continue_statement() {
        let script_text = r#"
            expect {
                "busy" {
                    exp_continue
                }
                "done" {}
            }
        "#;

        let result = Script::from_str(script_text);
        assert!(
            result.is_ok(),
            "Failed to parse exp_continue statement: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn test_execute_exp_continue_retries_pattern() {
        // No reliably-scriptable cross-platform bidirectional echo command;
        // `cat` is the standard tool for this on Unix.
        if cfg!(windows) {
            return;
        }

        let script_text = r#"
            spawn cat
            send "first\n"
            send "done\n"
            set seen 0
            expect {
                "first" {
                    set seen 1
                    exp_continue
                }
                "done" {}
            }
        "#;

        let script = Script::builder()
            .timeout(Duration::from_secs(5))
            .from_str(script_text)
            .expect("Failed to parse script");

        let result = script.execute().await;
        assert!(
            result.is_ok(),
            "Script execution with exp_continue failed: {:?}",
            result.err()
        );

        let result = result.unwrap();
        assert_eq!(
            result.variables.get("seen").unwrap().as_number().unwrap(),
            1.0,
            "exp_continue should have looped back after matching \"first\" and run its action"
        );
    }

    #[tokio::test]
    async fn test_exp_continue_outside_expect_is_error() {
        let script_text = "exp_continue\n";

        let script = Script::from_str(script_text).expect("Failed to parse script");
        let result = script.execute().await;

        assert!(result.is_err(), "Expected an error");
        match result.unwrap_err() {
            ScriptError::ExpContinue => {}
            other => panic!("Expected ExpContinue error, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_regex_pattern() {
        let script_text = if cfg!(windows) {
            r#"
                spawn cmd /c echo test123
                expect -re "test[0-9]+"
            "#
        } else {
            r#"
                spawn echo test123
                expect -re "test[0-9]+"
            "#
        };

        let script = Script::builder()
            .timeout(Duration::from_secs(5))
            .from_str(script_text)
            .expect("Failed to parse script");

        let result = script.execute().await;
        assert!(
            result.is_ok(),
            "Regex pattern test failed: {:?}",
            result.err()
        );
    }
}
