use camino::Utf8PathBuf;
use miette::IntoDiagnostic;

pub fn current_exe() -> miette::Result<Utf8PathBuf> {
    std::env::current_exe()
        .into_diagnostic()
        .and_then(|path| Utf8PathBuf::try_from(path).into_diagnostic())
}
