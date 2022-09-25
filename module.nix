{ config, lib, pkgs, ... }:
let
  cfg = config.services.webauthn-tiny;
in
{
  options = {
    services.webauthn-tiny = {
      enable = lib.mkEnableOption "webauthn-tiny server";
      relyingParty = {
        id = lib.mkOption {
          type = lib.types.str;
          description = ''
            TODO
          '';
          example = "mywebsite.com";
        };
        origin = lib.mkOption {
          type = lib.types.str;
          description = ''
            TODO
          '';
          example = "https://mywebsite.com";
        };
      };
      nginx = {
        enable = lib.mkEnableOption "nginx support";
        basePath = lib.mkOption {
          type = lib.types.str;
          description = ''
            The base path that will be prepended to each location for this service.
          '';
          default = "/auth";
        };
        virtualHost = lib.mkOption {
          type = lib.types.str;
          description = ''
            The virtual host that this service will serve on.
          '';
        };
        basicAuth = lib.mkOption {
          type = lib.types.attrsOf lib.types.str;
          description = ''
            A static mapping of usernames to passwords. WARNING: only use this for testing purposes.
          '';
          default = { };
          example = { myuser = "mypassword"; };
        };
        basicAuthFile = lib.mkOption {
          type = lib.types.nullOr lib.types.path;
          description = ''
            A path to an htpasswd file.
          '';
          default = null;
        };
      };
    };
  };
  config = lib.mkIf cfg.enable {
    services.nginx = lib.mkIf cfg.nginx.enable {
      enable = true;
      virtualHosts.${cfg.nginx.virtualHost} =
        let
          withProxy = { extraConfig ? "", ... }@args: args // {
            proxyPass = "http://[::1]:8080";
            extraConfig = extraConfig + ''
              proxy_set_header Host            $host;
              proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
              proxy_set_header X-Remote-User   $remote_user;
            '';
          };
        in
        {
          locations."= ${cfg.nginx.basePath}/api/validate" = withProxy {
            extraConfig = ''
              proxy_set_header Host            $host;
              proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
              proxy_set_header X-Remote-User   $remote_user;
            '';
          };
          locations."${cfg.nginx.basePath}/api" = withProxy {
            inherit (cfg.nginx) basicAuthFile basicAuth;
          };
          locations."${cfg.nginx.basePath}/" = {
            inherit (cfg.nginx) basicAuthFile basicAuth;
            alias = "${pkgs.webauthn-tiny.web-ui}/";
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
