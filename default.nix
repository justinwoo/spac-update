{ pkgs ? import <nixpkgs> {} }:

let
  spac-update-src = ./.;

  binary = pkgs.rustPlatform.buildRustPackage rec {
    name = "spac-update-rs";
    src = spac-update-src;
    version = "0.1.0";
    cargoSha256 = "0xfvx8k47wx417yj8s3g3dl724kjv5ni7gvlx1ai03kxgwjgi1mn";
  };

in pkgs.runCommand "spac-update" {
  name = "spac-update";
  buildInputs = [
    pkgs.makeWrapper
  ];
} ''
    mkdir -p $out/bin
    install -D -m555 -t $out/bin ${binary}/bin/spac-update

    echo "WARNING:"
    echo "You must provide your own Bower for this program to work."
    echo "I do not supply Bower because NixPkgs versions are not suitable for use with this project."
    echo "Please install Bower through npm i -g bower, after seting npm prefix to e.g. ~/.npm"

    wrapProgram $out/bin/spac-update \
      --prefix PATH : ${pkgs.lib.makeBinPath [
        pkgs.jq
      ]}

    mkdir -p $out/etc/bash_completion.d/
    cp ${spac-update-src}/spac-update-completion.bash $out/etc/bash_completion.d/spac-update-completion.bash
  ''
