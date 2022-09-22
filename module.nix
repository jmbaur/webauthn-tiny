{ config, lib, pkgs, ... }:
let
  cfg = config.services.webauthn-tiny;
in
{
  options = {
    services.webauthn-tiny = {
      enable = lib.mkEnableOption "webauthn-tiny server";
      userFile = lib.mkOption { type = lib.types.path; };
      credentialFile = lib.mkOption { type = lib.types.path; };
      domain = lib.mkOption { type = lib.types.str; };
      basicAuthFile = lib.mkOption { type = lib.types.nullOr lib.types.path; };
      basicAuth = lib.mkOption { type = types.attrsOf types.str; default = { }; };
      relyingParty = {
        id = lib.mkOption { type = lib.types.str; };
        origin = lib.mkOption { type = lib.types.str; };
      };
    };
  };
  config = lib.mkIf cfg.enable {
    services.nginx = {
      enable = true;
      virtualHosts."${cfg.domain}" = {
        basicAuthFile = cfg.basicAuthFile;
        basicAuth = cfg.basicAuth;
        locations."/validate".extraConfig = ''
          auth_basic off;
        '';
        locations."/" = {
          root = "${pkgs.webauthn-tiny-client}";
          tryFiles = "$uri /index.html =404";
        };
        locations."/api" = {
          proxyPass = "http://[::1]:8080";
          extraConfig = ''
            proxy_set_header Host            $host;
            proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
            proxy_set_header X-Remote-User   $remote_user;
          '';
        };
      };
    };

    systemd.services.webauthn-tiny = {
      enable = true;
      description = "webauthn-tiny (https://github.com/jmbaur/webauthn-tiny)";
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
