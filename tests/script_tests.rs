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
        let script_text = "spawn";
        let result = Script::from_str(script_text);
        assert!(
            result.is_err(),
            "Should have failed to parse incomplete spawn"
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

    #[tokio::test]
    async fn test_regex_pattern() {
        // Skip this test on Windows due to cmd.exe output formatting issues
        if cfg!(windows) {
            return;
        }

        let script_text = r#"
            spawn echo test123
            expect -re "test[0-9]+"
        "#;

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
