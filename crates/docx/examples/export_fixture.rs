//! Synthetic validation only. Never loads a workspace database.
use novel_storage::{ExportProfile, ProjectBundle};
use std::{env, fs, io::Read};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.len() != 2 && !(args.len() == 4 && args[2] == "--profile") {
        return Err("Usage: export_fixture <project-bundle.json> <output.docx> [--profile <profile.json>]".into());
    }
    let mut source = Vec::new();
    fs::File::open(&args[0])?.take(novel_docx::FILE_LIMIT as u64 + 1).read_to_end(&mut source)?;
    if source.len() > novel_docx::FILE_LIMIT { return Err("Fixture exceeds the export limit.".into()); }
    let bundle: ProjectBundle = serde_json::from_slice(&source)?;
    let bytes = if args.len() == 4 {
        let mut profile_source = Vec::new();
        fs::File::open(&args[3])?.take(64 * 1024 + 1).read_to_end(&mut profile_source)?;
        if profile_source.len() > 64 * 1024 { return Err("Export profile exceeds the 64 KiB limit.".into()); }
        let profile: ExportProfile = serde_json::from_slice(&profile_source)?;
        novel_docx::export_manuscript_with_profile(&bundle, &profile)?
    } else {
        novel_docx::export_manuscript(&bundle)?
    };
    fs::write(&args[1], bytes)?;
    println!("Synthetic DOCX exported.");
    Ok(())
}
