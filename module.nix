{ config, lib, pkgs, ... }:
with lib;
let
  cfg = config.services.webauthn-tiny;
  passwordFile = if (cfg.basicAuthFile != null) then cfg.basicAuthFile else
  (pkgs.runCommand "generated-password-file" { } (''
    touch $out
    salt=$(${pkgs.openssl}/bin/openssl rand -hex 16)
  '' + (concatStringsSep ";"
    (mapAttrsToList
      (username: password: ''
        echo ${username}:$(printf "${password}" | ${pkgs.libargon2}/bin/argon2 $salt -id -e) >> $out
      '')
      cfg.basicAuth))
  ));
  sessionSecretFile = if (cfg.sessionSecretFile != null) then cfg.sessionSecretFile else
  (pkgs.runCommand "generated-session-secret-file" { } "${pkgs.openssl}/bin/openssl rand -hex 64 > $out");
in
{
  options = {
    services.webauthn-tiny = {
      enable = mkEnableOption "webauthn-tiny server";
      package = mkPackageOption pkgs "webauthn-tiny" { };
      basicAuth = mkOption {
        type = types.attrsOf types.str;
        description = ''
          A static mapping of usernames to passwords. WARNING: only use this
          for testing purposes.
        '';
        default = { };
        example = { myuser = "mypassword"; };
      };
      basicAuthFile = mkOption {
        type = types.nullOr types.path;
        default = null;
        description = ''
          The path to a password file. This file must contain lines in the form
          of "<username>:<argon2_hash>". A valid Argon2 hash can be generated
          using the `libargon2` package like so: `argon2 <salt> -id -e`.
        '';
      };
      sessionSecretFile = mkOption {
        type = types.nullOr types.path;
        default = null;
        description = ''
          The path to a file containing a session secret (64 or more bytes).
          You can use `openssl rand -hex 64` to generate a session secret.
        '';
      };
      relyingParty = {
        id = mkOption {
          type = types.str;
          description = ''
            An ID that corresponds to the domain applicable for that Relying
            Party.
          '';
          example = "mywebsite.com";
        };
        origin = mkOption {
          type = types.str;
          description = ''
            The origin on which registrations for the Relying Party will take
            place.
          '';
          example = "https://mywebsite.com";
        };
        extraAllowedOrigins = mkOption {
          type = types.listOf types.str;
          default = [ ];
          description = ''
            Extra allowed origins that will be allowed for redirects and trusted
            by the webauthn instance.
          '';
          example = [ "https://subdomain.mywebsite.com" ];
        };
      };
      nginx = {
        enable = mkEnableOption "nginx support";
        virtualHost = mkOption {
          type = types.str;
          description = ''
            The virtual host that this service will serve on.
          '';
        };
        protectedVirtualHosts = mkOption {
          type = types.listOf types.str;
          description = ''
            A list of virtual hosts that will be protected by this webauthn
            server. This uses nginx's auth_request functionality.
          '';
          default = [ ];
        };
        enableACME = mkEnableOption "enable ACME on this virtual host";
        useACMEHost = mkOption {
          type = types.nullOr types.str;
          default = null;
          description = ''
            Whether to ask Let's Encrypt to sign a certificate for this vhost.
            Alternately, you can use an existing certificate through
            useACMEHost.
          '';
        };
      };
    };
  };
  config = mkIf cfg.enable {
    services.nginx = mkIf cfg.nginx.enable {
      enable = true;
      virtualHosts = genAttrs cfg.nginx.protectedVirtualHosts
        (_: {
          extraConfig = ''
            auth_request /auth;
            error_page 401 = @error401;
            auth_request_set $set_cookie $upstream_http_set_cookie;
            more_set_headers "Set-Cookie: $set_cookie";
          '';
          locations."= /auth" = {
            proxyPass = "http://[::1]:8080/api/validate";
            extraConfig = ''
              internal;
              proxy_pass_request_body off;
              proxy_set_header Content-Length "";
            '';
          };
          locations."@error401".return = "307 $scheme://${cfg.nginx.virtualHost}/authenticate?redirect_url=https://$http_host";
        }) // {
        ${cfg.nginx.virtualHost} = {
          inherit (cfg.nginx) enableACME useACMEHost;
          forceSSL = true; # webauthn is only available over HTTPS
          locations."/" = {
            proxyPass = "http://[::1]:8080";
            extraConfig = ''
              proxy_set_header Host            $host;
              proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
            '';
          };
        };
      };
    };

    systemd.services.webauthn-tiny = {
      enable = true;
      description = "webauthn-tiny (https://github.com/jmbaur/webauthn-tiny)";
      environment.WEBAUTHN_TINY_LOG = "info";
      serviceConfig = {
        StateDirectory = "webauthn-tiny";
        LoadCredential = [ "password-file:${passwordFile}" "session-secret-file:${sessionSecretFile}" ];
        ExecStart =
          escapeShellArgs ([
            "${pkgs.webauthn-tiny}/bin/webauthn-tiny"
            "--rp-id=${cfg.relyingParty.id}"
            "--rp-origin=${cfg.relyingParty.origin}"
            "--password-file=\${CREDENTIALS_DIRECTORY}/password-file"
            "--session-secret-file=\${CREDENTIALS_DIRECTORY}/session-secret-file"
          ] ++ (map (origin: "--extra-allowed-origin=${origin}") cfg.relyingParty.extraAllowedOrigins)
          );
        CapabilityBoundingSet = [ ];
        DeviceAllow = [ ];
        DynamicUser = true;
        LockPersonality = true;
        MemoryDenyWriteExecute = true;
        NoNewPrivileges = true;
        PrivateDevices = true;
        ProtectClock = true;
        ProtectControlGroups = true;
        ProtectHome = true;
        ProtectHostname = true;
        ProtectKernelLogs = true;
        ProtectKernelModules = true;
        ProtectKernelTunables = true;
        ProtectSystem = "strict";
        RemoveIPC = true;
        RestrictAddressFamilies = [ "AF_INET" "AF_INET6" ];
        RestrictNamespaces = true;
        RestrictRealtime = true;
        RestrictSUIDSGID = true;
        SystemCallArchitectures = "native";
      };
      wantedBy = [ "multi-user.target" ];
    };
  };
}
