//! Process spawning utilities

use crate::result::ExpectError;

/// Check if a child process is still alive
pub fn is_alive(child: &mut Box<dyn portable_pty::Child + Send>) -> Result<bool, ExpectError> {
    match child.try_wait() {
        Ok(Some(_)) => Ok(false), // Process exited
        Ok(None) => Ok(true),     // Still running
        Err(e) => Err(ExpectError::IoError(e)),
    }
}
