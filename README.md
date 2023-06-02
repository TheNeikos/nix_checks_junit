# nix_checks_junit

`nix_checks_junit` is a helper program that generates a junit report from your
flake `checks` output.

It does this by evaluating your flake and extracting all checks and
individually verifying if it builds or not. On failure, the log is appended for easier debugging.

## How to use it

Simply run `nix run github:TheNeikos/nix_checks_junit -- run-checks
--output-path <file>` to have it automatically generate a junit compatible file
of your checks. This file can then be processed further by your build pipeline.
