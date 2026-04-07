# Changelog

## [0.2.0](https://github.com/alisonjenkins/tf-format/compare/v0.1.0...v0.2.0) (2026-04-07)


### Features

* add ci workflow ([6243528](https://github.com/alisonjenkins/tf-format/commit/6243528d7bed34924614d69d2f34ec77c83dca93))
* add release workflow ([bcce328](https://github.com/alisonjenkins/tf-format/commit/bcce328a4103288ab4ab1d7ef102eeabf1f71f58))
* add tf-format github action ([2ae21ba](https://github.com/alisonjenkins/tf-format/commit/2ae21baa8eaae716751bda6e95bd35d5c7de87d9))
* **ci:** automate releases with release-please ([986a160](https://github.com/alisonjenkins/tf-format/commit/986a160ebfc2947eac6377b04ec7b71aacad9172))
* enforce trailing commas in multi-line arrays ([9c3a1c3](https://github.com/alisonjenkins/tf-format/commit/9c3a1c317b690e90daa584cd0d3f2f5bb90d47d2))
* expand single-line object literals that exceed line width ([901c64f](https://github.com/alisonjenkins/tf-format/commit/901c64fb2a1a8d091de5e77dea75883d8463ac56))
* hoist meta-arguments to top of resource/module/data blocks ([bb87768](https://github.com/alisonjenkins/tf-format/commit/bb877685f86cf7ca436ca43593253b9981fedb90))
* initial implementation of tf-format ([7558205](https://github.com/alisonjenkins/tf-format/commit/7558205c0057671814ce37c78697e56a26e30a9c))


### Bug Fixes

* add .direnv to gitignore ([73c5224](https://github.com/alisonjenkins/tf-format/commit/73c52247399133028fecc97e50d29ad1c7414258))
* add .envrc to use nix devshell ([3518f2c](https://github.com/alisonjenkins/tf-format/commit/3518f2c02482fcdc97b96bdbd132a859011be635))
* align '=' identically to terraform/opentofu fmt ([5377ca7](https://github.com/alisonjenkins/tf-format/commit/5377ca7ccad8fed27af5a738b0b29e20d7d8d4c2))
* correct indentation for objects inside arrays ([94f0794](https://github.com/alisonjenkins/tf-format/commit/94f0794fa7dcdc4926be39d6f4f86a31827d33f6))
* don't add a depth level for inline '[{ ... }]' array elements ([f6aba82](https://github.com/alisonjenkins/tf-format/commit/f6aba8242fa73091bf83742cd535ac3059155863))
* don't align '=' across multi-line object entries ([0709c95](https://github.com/alisonjenkins/tf-format/commit/0709c95be317da4825695e0e72ff5b912f36472d))
* emit newline before closing '}' on comma-terminated objects ([19189d3](https://github.com/alisonjenkins/tf-format/commit/19189d334d7125ed2a1d9606cb2392eae23ba584))
* measure quoted-string object keys without their decor ([96017c3](https://github.com/alisonjenkins/tf-format/commit/96017c30e00639ee7567015f481460916974f1a4))
* update flake ([a4f8dde](https://github.com/alisonjenkins/tf-format/commit/a4f8ddedc92911de3b175062339a377f96747dec))
