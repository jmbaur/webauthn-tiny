{ fetchurl, fetchgit, linkFarm, runCommand, gnutar }: rec {
  offline_cache = linkFarm "offline" packages;
  packages = [
    {
      name = "_github_webauthn_json___webauthn_json_2.0.1.tgz";
      path = fetchurl {
        name = "_github_webauthn_json___webauthn_json_2.0.1.tgz";
        url = "https://registry.yarnpkg.com/@github/webauthn-json/-/webauthn-json-2.0.1.tgz";
        sha512 = "9vjpjK3Qfd5FgvdueWYOGnR3TadG1dLQ2vHoL+un5JutH1fqR4LZaOWqHWGLmZz7NZodGle53GdXF9yhq4JWgA==";
      };
    }
    {
      name = "_rometools_cli_darwin_arm64___cli_darwin_arm64_0.9.2_next.tgz";
      path = fetchurl {
        name = "_rometools_cli_darwin_arm64___cli_darwin_arm64_0.9.2_next.tgz";
        url = "https://registry.yarnpkg.com/@rometools/cli-darwin-arm64/-/cli-darwin-arm64-0.9.2-next.tgz";
        sha512 = "7i/3sRwCsz5QzGarYpiggInciSO99mH2Qx3LZ8Mf+ia1jPWBaVtGiN+GimFZEcFnYhJ6EXMNYlAuX1nJyaRSVQ==";
      };
    }
    {
      name = "_rometools_cli_darwin_x64___cli_darwin_x64_0.9.2_next.tgz";
      path = fetchurl {
        name = "_rometools_cli_darwin_x64___cli_darwin_x64_0.9.2_next.tgz";
        url = "https://registry.yarnpkg.com/@rometools/cli-darwin-x64/-/cli-darwin-x64-0.9.2-next.tgz";
        sha512 = "Sy0cgqW86PT0TuyAgNHpalRXrKM5WZPnDtHKVS2QXA5Ad01dhQxPHl5SiF12ussJztHWjWDmMKAso/Uopz8ENw==";
      };
    }
    {
      name = "_rometools_cli_linux_arm64___cli_linux_arm64_0.9.2_next.tgz";
      path = fetchurl {
        name = "_rometools_cli_linux_arm64___cli_linux_arm64_0.9.2_next.tgz";
        url = "https://registry.yarnpkg.com/@rometools/cli-linux-arm64/-/cli-linux-arm64-0.9.2-next.tgz";
        sha512 = "WdhmT4sx1iGbjME8krEsYLmkgzJlNwv9yEjwIyhpAe87QngYF4w7KwdZeCAp28jkI8cp5vGYjTAHtbARRMUAcQ==";
      };
    }
    {
      name = "_rometools_cli_linux_x64___cli_linux_x64_0.9.2_next.tgz";
      path = fetchurl {
        name = "_rometools_cli_linux_x64___cli_linux_x64_0.9.2_next.tgz";
        url = "https://registry.yarnpkg.com/@rometools/cli-linux-x64/-/cli-linux-x64-0.9.2-next.tgz";
        sha512 = "Pgq0srYfXqgZ/Zlv2a3BzHhh+dewddWRrSWqqhjxcLLEm5IPVqfVmq0fUbNMZYm9M7DOJbUmcMdU6lPFdV044w==";
      };
    }
    {
      name = "_rometools_cli_win32_arm64___cli_win32_arm64_0.9.2_next.tgz";
      path = fetchurl {
        name = "_rometools_cli_win32_arm64___cli_win32_arm64_0.9.2_next.tgz";
        url = "https://registry.yarnpkg.com/@rometools/cli-win32-arm64/-/cli-win32-arm64-0.9.2-next.tgz";
        sha512 = "6+5R/IzJKxIXMefjQ03/5D+fKBSsynV9LvF3Ovtr0vsSfA3SdjRfsOlkH/bPOrWcLT96Fkdq8ATWd5Ah2NBQ6A==";
      };
    }
    {
      name = "_rometools_cli_win32_x64___cli_win32_x64_0.9.2_next.tgz";
      path = fetchurl {
        name = "_rometools_cli_win32_x64___cli_win32_x64_0.9.2_next.tgz";
        url = "https://registry.yarnpkg.com/@rometools/cli-win32-x64/-/cli-win32-x64-0.9.2-next.tgz";
        sha512 = "cFg4mGdkWcVajk+mKWjt5g/RBaxvCrJ8qBseLSIWEh8jHfpSTbOFc5Z/Mkv7QY1WWOEUBr0bX71iqZqfh3jfOA==";
      };
    }
    {
      name = "_rometools_wasm_bundler___wasm_bundler_0.9.2_next.tgz";
      path = fetchurl {
        name = "_rometools_wasm_bundler___wasm_bundler_0.9.2_next.tgz";
        url = "https://registry.yarnpkg.com/@rometools/wasm-bundler/-/wasm-bundler-0.9.2-next.tgz";
        sha512 = "sa2rpam4spyijJMGfqrxttN8QX67OyZjAb+JyFB5FGfNvd075nFdTBSuM177tgbbSwmt3n2MY23EV1aCLiGTmg==";
      };
    }
    {
      name = "_rometools_wasm_nodejs___wasm_nodejs_0.9.2_next.tgz";
      path = fetchurl {
        name = "_rometools_wasm_nodejs___wasm_nodejs_0.9.2_next.tgz";
        url = "https://registry.yarnpkg.com/@rometools/wasm-nodejs/-/wasm-nodejs-0.9.2-next.tgz";
        sha512 = "yUTxjYZfrqXQrszRUVK/lUfRJOD5g9wfZXeRBJ4FJllGCy1CvwwR9bS09oSLIZd8zMCC02XMBBW+OZJ1b6rZrA==";
      };
    }
    {
      name = "_rometools_wasm_web___wasm_web_0.9.2_next.tgz";
      path = fetchurl {
        name = "_rometools_wasm_web___wasm_web_0.9.2_next.tgz";
        url = "https://registry.yarnpkg.com/@rometools/wasm-web/-/wasm-web-0.9.2-next.tgz";
        sha512 = "iZpgD4n5f2tMeSwlF50k/d8RGBkdoi7wX9YtYTXFks7mN3Z5OU7IHC1LvY28yAv0EELwMFYaaZIvhIs0jozzKw==";
      };
    }
    {
      name = "rome___rome_0.9.2_next.tgz";
      path = fetchurl {
        name = "rome___rome_0.9.2_next.tgz";
        url = "https://registry.yarnpkg.com/rome/-/rome-0.9.2-next.tgz";
        sha512 = "ppc7Jg3oZfmVXvs28OynJBWa26dy8201QNH3vY8RlfdzxblLOb9+ovPgcmwceSURZMNz/HS+aTpuHW8T06ciHA==";
      };
    }
    {
      name = "typescript___typescript_4.8.4.tgz";
      path = fetchurl {
        name = "typescript___typescript_4.8.4.tgz";
        url = "https://registry.yarnpkg.com/typescript/-/typescript-4.8.4.tgz";
        sha512 = "QCh+85mCy+h0IGff8r5XWzOVSbBO+KfeYrMQh7NJ58QujwcE22u+NUSmUxqF+un70P9GXKxa2HCNiTTMJknyjQ==";
      };
    }
  ];
}
