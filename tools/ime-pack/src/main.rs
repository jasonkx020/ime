mod zh_lexicon;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use yc_pack::{build_langpack_dir, build_skin_dir, extract_manifest_from_pack, extract_skin_from_pack};

#[derive(Parser)]
#[command(name = "ime-pack", about = "Build and inspect LangPack / SkinPack assets")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Validate { dir: PathBuf },
    Build {
        #[arg(short, long)]
        output: PathBuf,
        dir: PathBuf,
    },
    BuildSkin {
        #[arg(short, long)]
        output: PathBuf,
        dir: PathBuf,
    },
    CompileLexicon {
        tsv: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    BuildZhLexicon {
        #[arg(long, default_value = "fixtures/cache/pinyin/phrase.txt")]
        phrase_pinyin: PathBuf,
        #[arg(long, default_value = "fixtures/cache/pinyin/char.txt")]
        char_pinyin: PathBuf,
        #[arg(long, default_value = "fixtures/langpacks/zh-pack-v1/lexicon/zh_words.sample.tsv")]
        sample_tsv: PathBuf,
        #[arg(long)]
        thuocl_dir: Option<PathBuf>,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        core_output: Option<PathBuf>,
        #[arg(long, default_value_t = 8000)]
        core_limit: usize,
        #[arg(long)]
        dat_output: Option<PathBuf>,
    },
    Inspect { file: PathBuf },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Validate { dir } => {
            let pack = dir.join("pack.toml");
            let skin = dir.join("skin.toml");
            if pack.exists() {
                let text = std::fs::read_to_string(&pack).expect("read pack.toml");
                let _: yc_pack::PackToml =
                    toml::from_str(&text).expect("invalid pack.toml");
                println!("OK: {}", dir.display());
            } else if skin.exists() {
                let text = std::fs::read_to_string(&skin).expect("read skin.toml");
                let _: yc_pack::SkinToml = toml::from_str(&text).expect("invalid skin.toml");
                println!("OK: {}", dir.display());
            } else {
                eprintln!("no pack.toml or skin.toml in {}", dir.display());
                std::process::exit(1);
            }
        }
        Commands::Build { output, dir } => {
            let out = build_langpack_dir(&dir, &output).expect("build");
            println!(
                "Built {} id={} sig={}",
                out.path.display(),
                out.manifest.id,
                out.signature
            );
        }
        Commands::BuildSkin { output, dir } => {
            let m = build_skin_dir(&dir, &output).expect("build skin");
            println!("Built skin {} ({})", m.id, m.name);
        }
        Commands::CompileLexicon { tsv, output } => {
            let dat = yc_lexicon::compile_tsv_to_dat(&tsv).expect("compile lexicon");
            if let Some(parent) = output.parent() {
                std::fs::create_dir_all(parent).expect("create output dir");
            }
            std::fs::write(&output, dat).expect("write dat");
            println!("Compiled {} -> {}", tsv.display(), output.display());
        }
        Commands::BuildZhLexicon {
            phrase_pinyin,
            char_pinyin,
            sample_tsv,
            thuocl_dir,
            output,
            core_output,
            core_limit,
            dat_output,
        } => {
            let count = zh_lexicon::build_zh_lexicon(&zh_lexicon::BuildZhLexiconOptions {
                phrase_pinyin,
                char_pinyin,
                sample_tsv,
                thuocl_dir,
                output_tsv: output.clone(),
                core_tsv: core_output,
                core_limit,
            })
            .expect("build zh lexicon");
            if let Some(dat) = dat_output {
                let bytes = yc_lexicon::compile_tsv_to_dat(&output).expect("compile dat");
                if let Some(parent) = dat.parent() {
                    std::fs::create_dir_all(parent).expect("create dat dir");
                }
                std::fs::write(&dat, bytes).expect("write dat");
                println!("Compiled dat -> {} ({} entries)", dat.display(), count);
            }
        }
        Commands::Inspect { file } => {
            if file.extension().and_then(|s| s.to_str()) == Some("imeskin") {
                let m = extract_skin_from_pack(&file).expect("inspect skin");
                println!("{m:#?}");
            } else {
                let m = extract_manifest_from_pack(&file).expect("inspect pack");
                println!("{m:#?}");
            }
        }
    }
}
