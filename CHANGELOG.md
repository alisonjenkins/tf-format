# Changelog

## [0.4.9](https://github.com/alisonjenkins/tf-format/compare/v0.4.8...v0.4.9) (2026-06-18)


### Bug Fixes

* **formatter:** align trailing comments by column ([498f28e](https://github.com/alisonjenkins/tf-format/commit/498f28e029bacd97ad34c2f12086604fdd78c8fc))

## [0.4.8](https://github.com/alisonjenkins/tf-format/compare/v0.4.7...v0.4.8) (2026-06-17)


### Bug Fixes

* **cli:** default to the current directory and fail on empty glob matches ([560eefa](https://github.com/alisonjenkins/tf-format/commit/560eefad99f19ec40018199abc719ae6ed1583b7))
* **cli:** print the diff when --check and --diff are combined ([6e64eb8](https://github.com/alisonjenkins/tf-format/commit/6e64eb8227ed8abc214c18fe91aef3176e7cdd8d))
* **cli:** reject positional file arguments with --stdin ([34559b5](https://github.com/alisonjenkins/tf-format/commit/34559b53716c935d2806ae64a2f5f30e145a6951))
* **formatter:** align `=` by rune count, not byte length ([09baf51](https://github.com/alisonjenkins/tf-format/commit/09baf515dbf0fb45848313d83b51c076dfa04d3b))
* **formatter:** detect blank lines in CRLF decor ([dec9610](https://github.com/alisonjenkins/tf-format/commit/dec9610248642195b3c3d49dafd78b5774d0959c))
* **formatter:** keep a file-header comment at the top when sorting blocks ([2ae1417](https://github.com/alisonjenkins/tf-format/commit/2ae1417bd573b370d5160a0b46fde685ffac97ad))
* **formatter:** keep inline comments on comma-terminated object entries ([5eab8bf](https://github.com/alisonjenkins/tf-format/commit/5eab8bf8b0d417ae975cc9538e5704e8e6d64603))
* **formatter:** keep inline object-entry comments in opinionated mode ([4fad4cf](https://github.com/alisonjenkins/tf-format/commit/4fad4cf72bde188f556ad95313618d71c8c55d6e))
* **formatter:** no forced blank before a top-level attr run in minimal mode ([0379514](https://github.com/alisonjenkins/tf-format/commit/0379514998e6fb7c6a07c078ab728d121c90fcee))
* **formatter:** normalize stale heredoc `=` padding in opinionated mode ([11996cf](https://github.com/alisonjenkins/tf-format/commit/11996cf0fcca13e5c632dbdc41a0a68df3263ca7))
* **formatter:** preserve blank after an inline object-open comment (minimal) ([ee38518](https://github.com/alisonjenkins/tf-format/commit/ee385184b3991a32e620c7604d9192ea6be63eb5))
* **formatter:** walk for-cond and index exprs when restoring heredoc markers ([17eb5b8](https://github.com/alisonjenkins/tf-format/commit/17eb5b8417b6f0c712e4dc5178cbcae3693e92cc))
* **lib:** ignore heredoc openers inside comments in the text scanners ([7eb1c22](https://github.com/alisonjenkins/tf-format/commit/7eb1c224a401c480b5c701a293552b68cb48a73a))
* **lib:** refuse to format input that hcl-edit cannot represent ([96ca21e](https://github.com/alisonjenkins/tf-format/commit/96ca21ed4ac295563c26ec86cd4f485a475e62f5))
* **opinionated:** hoist dynamic-block for_each/iterator/labels before content ([0b7230c](https://github.com/alisonjenkins/tf-format/commit/0b7230cd064e882f8cd8f9a2ac1564dea73be121))
* **opinionated:** strip trailing commas from multi-line map entries ([4d8223d](https://github.com/alisonjenkins/tf-format/commit/4d8223dfaeaf91e52670ef75a276cce9477123ab))

## [0.4.7](https://github.com/alisonjenkins/tf-format/compare/v0.4.6...v0.4.7) (2026-06-08)


### Bug Fixes

* **formatter:** strip stale =-alignment from multi-line attrs and preserve tofu layout ([4875126](https://github.com/alisonjenkins/tf-format/commit/48751264e8094b679f5eb41c42d3bf34e2bde19c))

## [0.4.6](https://github.com/alisonjenkins/tf-format/compare/v0.4.5...v0.4.6) (2026-06-06)


### Bug Fixes

* **formatter:** preserve leading + inline object comments (minimal-mode parity) ([#48](https://github.com/alisonjenkins/tf-format/issues/48)) ([cd3a090](https://github.com/alisonjenkins/tf-format/commit/cd3a09095ab347a260fd8c6519b60a910090e69b))

## [0.4.5](https://github.com/alisonjenkins/tf-format/compare/v0.4.4...v0.4.5) (2026-06-05)


### Bug Fixes

* **formatter:** keep single space in one-line block bodies ([42ca9b1](https://github.com/alisonjenkins/tf-format/commit/42ca9b13fcf8729cff51ff64ea694efd38c8605e))
* **formatter:** silence clippy too-many-arguments on format_structure_group ([4a0dd1e](https://github.com/alisonjenkins/tf-format/commit/4a0dd1e21900466fff4a91582227ce0e638c33b2))

## [0.4.4](https://github.com/alisonjenkins/tf-format/compare/v0.4.3...v0.4.4) (2026-06-03)


### Bug Fixes

* **formatter:** preserve `<<-` heredoc marker dropped by hcl-edit ([#43](https://github.com/alisonjenkins/tf-format/issues/43)) ([#45](https://github.com/alisonjenkins/tf-format/issues/45)) ([ba8b6a1](https://github.com/alisonjenkins/tf-format/commit/ba8b6a119ee2df548676f2ed2de4fa6a6600add6))
* **formatter:** tidy blank lines in arrays and before closing braces ([#35](https://github.com/alisonjenkins/tf-format/issues/35)) ([#42](https://github.com/alisonjenkins/tf-format/issues/42)) ([3532bd9](https://github.com/alisonjenkins/tf-format/commit/3532bd93d74c437b22854cf1c0c87d94549f327c))

## [0.4.3](https://github.com/alisonjenkins/tf-format/compare/v0.4.2...v0.4.3) (2026-06-03)


### Bug Fixes

* **deps:** update rust crate similar to v3 ([#40](https://github.com/alisonjenkins/tf-format/issues/40)) ([f48fa3e](https://github.com/alisonjenkins/tf-format/commit/f48fa3ef9db8adcb70f75f5a5752f43bed53ba31))
* **formatter:** align heredoc openers in minimal mode (tofu parity) ([#41](https://github.com/alisonjenkins/tf-format/issues/41)) ([8fbf35a](https://github.com/alisonjenkins/tf-format/commit/8fbf35a7b1ca77e4462d47658924e33b0c72b771))
* resolve audit backlog — data-loss bugs, CLI robustness, coverage ([#38](https://github.com/alisonjenkins/tf-format/issues/38)) ([bfbd3e3](https://github.com/alisonjenkins/tf-format/commit/bfbd3e39731f5a77ee2d614b23fd6cc41fa7ade6))

## [0.4.2](https://github.com/alisonjenkins/tf-format/compare/v0.4.1...v0.4.2) (2026-05-26)


### Bug Fixes

* **formatter:** block-type-aware priority attribute hoisting ([#30](https://github.com/alisonjenkins/tf-format/issues/30)) ([36992bf](https://github.com/alisonjenkins/tf-format/commit/36992bf81a485534291ef642b743b8caa01d77ea))

## [0.4.1](https://github.com/alisonjenkins/tf-format/compare/v0.4.0...v0.4.1) (2026-04-29)


### Bug Fixes

* **formatter:** match tofu fmt for `:` object assignment (issue [#18](https://github.com/alisonjenkins/tf-format/issues/18)) ([6512daa](https://github.com/alisonjenkins/tf-format/commit/6512daa96ae5f181002c3e4445109f22c486276e))

## [0.4.0](https://github.com/alisonjenkins/tf-format/compare/v0.3.0...v0.4.0) (2026-04-29)


### Features

* add pre-commit hook support ([ff574fa](https://github.com/alisonjenkins/tf-format/commit/ff574fa1ce17357a57ce2042c1ec9595c186ee5b))

## [0.3.0](https://github.com/alisonjenkins/tf-format/compare/v0.2.1...v0.3.0) (2026-04-29)


### Features

* format top-level attributes in tfvars files ([6556a96](https://github.com/alisonjenkins/tf-format/commit/6556a96757b901f3a88f8fd8fd783c8f11f4be5b))
* minimal style — terraform fmt / tofu fmt parity mode ([937610e](https://github.com/alisonjenkins/tf-format/commit/937610ed24e031d496fdf2ff82e762879ab397ac))
* **opinionated:** collapse author blank lines for full reflow ([ec711e4](https://github.com/alisonjenkins/tf-format/commit/ec711e4e9404011e8acdc7de7af1e93c801e6228))
* preserve blank-line-separated alignment groups ([9d52488](https://github.com/alisonjenkins/tf-format/commit/9d5248837f47d7491727468b514b1f779de00de5))
* recurse format_expression into compound expressions ([9a4a8ba](https://github.com/alisonjenkins/tf-format/commit/9a4a8ba9635e8a29b2a8afbfca72e4140b3fc9fe))


### Bug Fixes

* **formatter:** for-expression value position needs depth+1 indent ([4a8abff](https://github.com/alisonjenkins/tf-format/commit/4a8abffcfe644b30a8251a268e6fa19191372358))
* **formatter:** FuncCall args at depth+1 when multi-line ([3d67d19](https://github.com/alisonjenkins/tf-format/commit/3d67d19ba26711dc01abf49bbbb0a8d1c0d97351))

## [0.2.1](https://github.com/alisonjenkins/tf-format/compare/v0.2.0...v0.2.1) (2026-04-07)


### Bug Fixes

* **ci:** cross-compile darwin with cargo-zigbuild ([7f686b5](https://github.com/alisonjenkins/tf-format/commit/7f686b50571203e7f26d06d5a7d33b7edb51a519))
* **ci:** dispatch release workflow from release-please ([c274cf0](https://github.com/alisonjenkins/tf-format/commit/c274cf0b6fcd69d11687b620db9444d5d86a6752))

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
