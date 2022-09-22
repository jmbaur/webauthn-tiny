inputs:
{ config, lib, pkgs, ... }:
let
  cfg = config.services.webauthn-tiny;

  # locationOptions = { config, ... }: {
  #   options = {
  #     webauthnProtect = lib.mkOption {
  #       type = lib.types.bool;
  #       default = false;
  #     };
  #     webauthnPath = lib.mkOption {
  #       type = lib.types.str;
  #       default = "/session";
  #     };
  #   };
  #   config.extraConfig = lib.mkIf config.webauthnProtect ''
  #     auth_request ${config.webauthnPath}
  #   '';
  # };
  virtualHostOptions = { config, ... }: {
    options = {
      webauthnProtect = lib.mkOption {
        type = lib.types.bool;
        default = false;
      };
      webauthnPath = lib.mkOption {
        type = lib.types.str;
        default = "/session";
      };
      # locations = lib.mkOption {
      #   type = lib.attrsOf (lib.submodule locationOptions);
      # };
    };
    config = lib.mkIf config.webauthnProtect {
      locations."= ${config.webauthnPath}" = {
        proxyPass = "http://localhost:8080/session";
        extraConfig = ''
          proxy_pass_request_body off;
          proxy_set_header Content-Length "";
          proxy_set_header X-Original-URI $request_uri;
        '';
      };
    };
  };
in
{
  options = {
    services.webauthn-tiny = {
      enable = lib.mkEnableOption "webauthn-tiny server";
      userFile = lib.mkOption { type = lib.types.path; };
      credentialFile = lib.mkOption { type = lib.types.path; };
      relyingParty = {
        id = lib.mkOption { type = lib.types.str; };
        origin = lib.mkOption { type = lib.types.str; };
      };
    };

    services.nginx.virtualHosts = lib.mkOption {
      type = lib.attrsOf (lib.submodule virtualHostOptions);
    };
  };
  config = lib.mkIf cfg.enable {
    assertions = [ ];

    nixpkgs.overlays = [ inputs.self.overlays.default ];

    systemd.services.webauthn-tiny = {
      enable = true;
      description = "webauthn-tiny (https://github.com/jmbaur/webauthn-tiny)";
      serviceConfig = {
        ProtectSystem = true;
        ProtectHome = true;
        DynamicUser = true;
        ExecStart = "${pkgs.webauthn-tiny}/bin/webauthn-tiny serve --userfile=${cfg.userFile} --credentialfile=${cfg.credentialFile} --id=${cfg.relyingParty.id} --origin=${cfg.relyingParty.origin}";
      };
      wantedBy = [ "multi-user.target" ];
    };
  };
}
