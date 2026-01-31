//! ANSI escape sequence stripping

/// Strip ANSI escape sequences from a byte slice
pub fn strip_ansi(data: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(data.len());
    let mut i = 0;

    while i < data.len() {
        if data[i] == b'\x1b' && i + 1 < data.len() {
            // ESC sequence detected
            match data[i + 1] {
                b'[' => {
                    // CSI (Control Sequence Introducer)
                    i += 2;
                    // Skip until we find a letter (the command)
                    while i < data.len() {
                        let ch = data[i];
                        i += 1;
                        if ch.is_ascii_alphabetic() {
                            break;
                        }
                    }
                }
                b']' => {
                    // OSC (Operating System Command)
                    i += 2;
                    // Skip until we find BEL (\x07) or ST (ESC \)
                    while i < data.len() {
                        if data[i] == b'\x07' {
                            i += 1;
                            break;
                        }
                        if data[i] == b'\x1b' && i + 1 < data.len() && data[i + 1] == b'\\' {
                            i += 2;
                            break;
                        }
                        i += 1;
                    }
                }
                b'(' | b')' => {
                    // Character set selection (ESC ( X or ESC ) X)
                    // Skip ESC, '(' or ')', and the character set designator
                    if i + 2 < data.len() {
                        i += 3;
                    } else {
                        i = data.len();
                    }
                }
                _ => {
                    // Other escape sequences - skip 2 chars
                    i += 2;
                }
            }
        } else {
            result.push(data[i]);
            i += 1;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_csi() {
        let input = b"Hello \x1b[31mred\x1b[0m world";
        let output = strip_ansi(input);
        assert_eq!(output, b"Hello red world");
    }

    #[test]
    fn test_strip_osc() {
        let input = b"Hello \x1b]0;Title\x07 world";
        let output = strip_ansi(input);
        assert_eq!(output, b"Hello  world");
    }

    #[test]
    fn test_no_ansi() {
        let input = b"Hello world";
        let output = strip_ansi(input);
        assert_eq!(output, b"Hello world");
    }

    #[test]
    fn test_multiple_sequences() {
        let input = b"\x1b[1mBold\x1b[0m and \x1b[4munderline\x1b[0m";
        let output = strip_ansi(input);
        assert_eq!(output, b"Bold and underline");
    }
}
