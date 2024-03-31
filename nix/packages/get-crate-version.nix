{
  lib,
  writeShellApplication,
  gayrat,
}:
writeShellApplication {
  name = "get-crate-version";

  text = ''
    echo ${lib.escapeShellArg gayrat.version}
  '';
}
