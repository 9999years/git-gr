use camino::Utf8PathBuf;

/// Gets a temporary `ssh` `ControlPath` file.
///
/// This path is persistent and truncated to 87 (???) bytes:
/// <https://unix.stackexchange.com/questions/367008/why-is-socket-path-length-limited-to-a-hundred-chars>
pub fn ssh_control_path(mut name: &str) -> Utf8PathBuf {
    const SIZE_LIMIT: usize = 87;
    let tmpdir = Utf8PathBuf::from("/tmp");
    let total_len = tmpdir.as_str().len() + 1 + name.len();
    if total_len > SIZE_LIMIT {
        let truncate = total_len - SIZE_LIMIT;
        // If your hostname contains non-ASCII this will explode.
        name = &name[..name.len() - truncate];
    }
    tmpdir.join(name)
}
