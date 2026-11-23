`.text` is 12,321,862 bytes; the symbol table names 12,238,612 of them (99.3%).

| section | bytes |
|---|---:|
| `Total` | 15,186,380 |
| `.text` | 12,321,862 |
| `.rodata` | 980,556 |
| `.eh_frame` | 724,576 |
| `.gcc_except_table` | 439,852 |
| `.rela.dyn` | 316,632 |
| `.data.rel.ro` | 263,616 |
| `.eh_frame_hdr` | 82,412 |
| `.bss` | 37,664 |
| `.data` | 15,968 |

| crate | code bytes | of the named code (%) |
|---|---:|---:|
| `cigscript` | 4,997,274 | 40.8 |
| `core` | 1,393,607 | 11.4 |
| `zstd (C)` | 821,181 | 6.7 |
| `zbus` | 719,032 | 5.9 |
| `zvariant` | 407,684 | 3.3 |
| `alloc` | 400,463 | 3.3 |
| `std` | 389,001 | 3.2 |
| `regex_automata` | 284,032 | 2.3 |
| `the C library and other unprefixed symbols` | 269,361 | 2.2 |
| `serde_json` | 263,042 | 2.1 |
| `clap_builder` | 248,780 | 2.0 |
| `cig_pass` | 227,395 | 1.9 |
| `secret_service` | 205,338 | 1.7 |
| `regex_syntax` | 173,799 | 1.4 |
| `aho_corasick` | 158,983 | 1.3 |
| `hashbrown` | 147,821 | 1.2 |
| `toml_edit` | 126,550 | 1.0 |
| `chrono` | 72,737 | 0.6 |
| `keyring` | 61,043 | 0.5 |
| `cig_bytecode` | 53,022 | 0.4 |
| `num_bigint` | 52,911 | 0.4 |
| `tar` | 45,910 | 0.4 |
| `miniz_oxide` | 45,760 | 0.4 |
| `blake3` | 45,571 | 0.4 |
| `globset` | 44,199 | 0.4 |
| `gimli` | 38,410 | 0.3 |
| `curve25519_dalek` | 38,165 | 0.3 |
| `sha2` | 30,890 | 0.3 |
| `async_task` | 29,592 | 0.2 |
| `serde` | 26,734 | 0.2 |
| the other 82 | 420,325 | 3.4 |

| module of `cigscript` | code bytes | of `cigscript` (%) |
|---|---:|---:|
| `cli` | 2,003,764 | 40.1 |
| `burn` | 1,282,285 | 25.7 |
| `stdlib` | 522,360 | 10.5 |
| `lower` | 295,935 | 5.9 |
| `vm` | 114,394 | 2.3 |
| `interp` | 109,454 | 2.2 |
| `syntax` | 96,979 | 1.9 |
| `crash` | 88,533 | 1.8 |
| `pass` | 78,926 | 1.6 |
| `errors` | 58,541 | 1.2 |
| `repro` | 56,264 | 1.1 |
| `tracker` | 41,561 | 0.8 |
| `check` | 39,065 | 0.8 |
| `program` | 36,310 | 0.7 |
| `doctor` | 31,074 | 0.6 |
| `tripwire` | 28,020 | 0.6 |
| `humidor` | 23,051 | 0.5 |
| `update` | 19,913 | 0.4 |
| `ashtray` | 13,263 | 0.3 |
| `value` | 12,735 | 0.3 |
| `config` | 11,726 | 0.2 |
| `diagnostics` | 11,584 | 0.2 |
| `since` | 6,124 | 0.1 |
| `pin` | 5,849 | 0.1 |
| `installer` | 3,908 | 0.1 |
| `timings` | 3,760 | 0.1 |
| `trial` | 1,896 | 0.0 |

| the 25 largest functions | bytes |
|---|---:|
| `<cigscript::cli::Command as clap_builder::derive::Subcommand>::augment_subcommands` | 191,263 |
| `cigscript::cli::tin::tin_cmd` | 88,304 |
| `<cigscript::burn::Kernel>::pack_watch_begin` | 83,583 |
| `cigscript::cli::dispatch` | 82,207 |
| `<cigscript::cli::pass::PassCommand as clap_builder::derive::Subcommand>::augment_subcommands` | 61,100 |
| `<cigscript::cli::tin::TinCommand as clap_builder::derive::Subcommand>::augment_subcommands` | 57,212 |
| `<cigscript::cli::BenchArgs as clap_builder::derive::Args>::augment_args` | 55,251 |
| `<cigscript::burn::Op as serde_core::de::Deserialize>::deserialize::<serde_json::value::Value>` | 55,019 |
| `<cigscript::burn::Kernel>::pack_watch_end` | 54,973 |
| `cigscript::vm::run_frame` | 52,932 |
| `<zbus::connection::builder::Builder>::build_::{closure#0}` | 48,291 |
| `cigscript::cli::run::run_source_inner` | 48,180 |
| `cigscript::cli::doctor::product_doctor` | 44,949 |
| `cigscript::cli::runs::unburn_stretch` | 43,772 |
| `cigscript::cli::pass::pass_cmd` | 41,121 |
| `<cig_pass::store::Store>::compact_impl` | 41,038 |
| `cigscript::cli::bench::watch::cycle` | 38,013 |
| `cigscript::errors::diagnostic_schema` | 37,473 |
| `cigscript::cli::run::trial_before_run` | 36,410 |
| `cigscript::cli::crash::crash` | 35,047 |
| `<cigscript::burn::foreign::SrcOp as serde_core::de::Deserialize>::deserialize::<serde::private::de::FlatMapDes` | 33,333 |
| `regex_automata::meta::strategy::new` | 32,982 |
| `<cigscript::syntax::parser::Parser>::stmt` | 30,884 |
| `cigscript::burn::tinparcel::write` | 30,787 |
| `<secret_service::blocking::SecretService>::connect` | 30,764 |
