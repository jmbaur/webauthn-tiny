{ config, lib, pkgs, ... }:
let
  cfg = config.services.webauthn-tiny;
in
{
  options = {
    # TODO(jared): add descriptions to options
    services.webauthn-tiny = {
      enable = lib.mkEnableOption "webauthn-tiny server";
      userFile = lib.mkOption { type = lib.types.path; };
      credentialFile = lib.mkOption { type = lib.types.path; };
      domain = lib.mkOption { type = lib.types.str; };
      basicAuthFile = lib.mkOption {
        type = lib.types.nullOr lib.types.path;
        default = null;
      };
      basicAuth = lib.mkOption {
        type = lib.types.attrsOf lib.types.str;
        default = { };
      };
      relyingParty = {
        id = lib.mkOption { type = lib.types.str; };
        origin = lib.mkOption { type = lib.types.str; };
      };
    };
  };
  config = lib.mkIf cfg.enable {
    services.nginx = {
      enable = true;
      virtualHosts."${cfg.domain}" =
        let
          withProxy = { extraConfig ? "" }@args: args // {
            proxyPass = "http://[::1]:8080";
            extraConfig = extraConfig + ''
              proxy_set_header Host            $host;
              proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
              proxy_set_header X-Remote-User   $remote_user;
            '';
          };
        in
        {
          inherit (cfg) basicAuthFile basicAuth;
          locations."= /api/validate" = withProxy {
            extraConfig = ''
              auth_basic off;
              proxy_set_header Host            $host;
              proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
              proxy_set_header X-Remote-User   $remote_user;
            '';
          };
          locations."/api" = withProxy { };
          locations."/" = {
            root = "${pkgs.webauthn-tiny.web-ui}";
            tryFiles = "$uri /index.html =404";
          };
        };
    };

    systemd.services.webauthn-tiny = {
      enable = true;
      description = "webauthn-tiny (https://github.com/jmbaur/webauthn-tiny)";
      environment.WEBAUTHN_TINY_LOG = "info";
      serviceConfig = {
        StateDirectory = "webauthn-tiny";
        ProtectSystem = true;
        ProtectHome = true;
        DynamicUser = true;
        ExecStart = "${pkgs.webauthn-tiny}/bin/webauthn-tiny --id=${cfg.relyingParty.id} --origin=${cfg.relyingParty.origin}";
      };
      wantedBy = [ "multi-user.target" ];
    };
  };
}
