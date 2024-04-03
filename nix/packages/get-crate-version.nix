{
  lib,
  writeShellApplication,
  git-gr,
}:
writeShellApplication {
  name = "get-crate-version";

  text = ''
    echo ${lib.escapeShellArg git-gr.version}
  '';
}
